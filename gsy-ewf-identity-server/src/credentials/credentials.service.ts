import { Injectable, Logger, BadRequestException, UnauthorizedException } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { v4 as uuidv4 } from 'uuid';
import * as ethers from 'ethers';
import { Keys } from '@ew-did-registry/keys';
import { Credential, CredentialStatus } from '../database/schemas/credential.schema';
import { User } from '../database/schemas/user.schema';
import { AuditService } from '../audit/audit.service';
import { DIDService } from '../did/did.service';
import { AuditAction } from '../database/schemas';
import { CredentialIssuanceResponse } from './dto/credential-issuance.dto';
import { CredentialVerificationResponse } from './dto/credential-verification.dto';
import { verifySubstrateSignature, formatSubstrateSigningMessage } from './utils/substrate-verification';

@Injectable()
export class CredentialsService {
  private readonly logger = new Logger(CredentialsService.name);
  private issuerDid: string;
  private issuerKeys: Keys;

  constructor(
    private configService: ConfigService,
    @InjectModel(Credential.name) private credentialModel: Model<Credential>,
    @InjectModel(User.name) private userModel: Model<User>,
    private didService: DIDService,
    private auditService: AuditService,
  ) {
    this.initializeIssuer();
  }

  private initializeIssuer() {
    try {
      const privateKey = this.configService.get<string>('ewc.issuerPrivateKey');
      const publicKey = this.configService.get<string>('ewc.issuerPublicKey');
      
      this.issuerKeys = new Keys({ privateKey, publicKey });
      const address = this.issuerKeys.getAddress();
      this.issuerDid = `did:ethr:${address}`;
      
      this.logger.log('Credential service initialized successfully');
    } catch (error) {
      this.logger.error(`Failed to initialize credential service: ${error.message}`);
      throw new Error('Failed to initialize credential service');
    }
  }

  async issueCredential(
    did: string,
    gsyDexAddress: string,
    challenge: string,
    didSignature: string,
    substrateSignature: string,
    req?: any,
  ): Promise<CredentialIssuanceResponse> {
    try {
      // Check if the DID is registered
      const isRegistered = await this.didService.isDIDRegistered(did);
      if (!isRegistered) {
        throw new BadRequestException('DID is not registered');
      }

      // Verify the DID signature
      const recoveredAddress = ethers.verifyMessage(challenge, didSignature);
      const didAddress = did.split(':')[2];
      
      if (recoveredAddress.toLowerCase() !== didAddress.toLowerCase()) {
        await this.auditService.log(
          AuditAction.CREDENTIAL_ISSUED,
          did,
          req,
          { gsyDexAddress, error: 'Invalid DID signature' },
          gsyDexAddress,
          false,
        );
        throw new UnauthorizedException('Invalid DID signature');
      }

      // Format the challenge for Substrate signature verification
      // This should match how the challenge was presented to the user for signing
      const formattedChallenge = formatSubstrateSigningMessage(challenge);
      
      // Verify the Substrate signature
      const isSubstrateSignatureValid = await verifySubstrateSignature(
        formattedChallenge,
        substrateSignature,
        gsyDexAddress,
      );
      
      if (!isSubstrateSignatureValid) {
        await this.auditService.log(
          AuditAction.CREDENTIAL_ISSUED,
          did,
          req,
          { gsyDexAddress, error: 'Invalid Substrate signature' },
          gsyDexAddress,
          false,
        );
        throw new UnauthorizedException('Invalid Substrate signature');
      }

      // Create a W3C-compliant Verifiable Credential
      const id = `urn:uuid:${uuidv4()}`;
      const issuanceDate = new Date().toISOString();
      const expirationDate = new Date();
      expirationDate.setFullYear(expirationDate.getFullYear() + 1); // 1 year validity
      
      const credential = {
        '@context': [
          'https://www.w3.org/2018/credentials/v1',
          'https://energyweb.org/contexts/gsy-dex-v1.jsonld',
        ],
        id,
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: this.issuerDid,
        issuanceDate,
        expirationDate: expirationDate.toISOString(),
        credentialSubject: {
          id: did,
          accountLink: {
            gsyDexAddress,
            chain: 'GSYDex',
          },
        },
      };

      // Sign the credential
      const credentialString = JSON.stringify(credential);
      
      // Create a wallet from private key
      const wallet = new ethers.Wallet(this.issuerKeys.privateKey);
      const signature = await wallet.signMessage(credentialString);
      
      // Add the proof to the credential
      const credentialWithProof = {
        ...credential,
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: issuanceDate,
          verificationMethod: `${this.issuerDid}#controller`,
          proofPurpose: 'assertionMethod',
          jws: signature,
        },
      };

      // Store the credential in the database
      const credentialRecord = new this.credentialModel({
        id,
        did,
        gsyDexAddress,
        credentialSubject: credential.credentialSubject,
        credential: credentialWithProof,
        status: CredentialStatus.ACTIVE,
        expirationDate,
      });
      await credentialRecord.save();

      // Update the user record
      await this.userModel.findOneAndUpdate(
        { did },
        {
          $set: {
            gsyDexAddress: gsyDexAddress,
            hasVerifiedCredential: true,
          },
        },
        { new: true, upsert: true },
      );

      // Log the credential issuance
      await this.auditService.log(
        AuditAction.CREDENTIAL_ISSUED,
        did,
        req,
        { credentialId: id, gsyDexAddress },
        gsyDexAddress,
      );

      return {
        id,
        credential: credentialWithProof,
      };
    } catch (error) {
      this.logger.error(`Failed to issue credential: ${error.message}`);
      if (error instanceof BadRequestException || error instanceof UnauthorizedException) {
        throw error;
      }
      throw new Error(`Failed to issue credential: ${error.message}`);
    }
  }

  async verifyCredential(
    credential: Record<string, any>,
    req?: any,
  ): Promise<CredentialVerificationResponse> {
    try {
      // Basic validation of the credential
      if (!credential || !credential.id || !credential.issuer ||
          !credential.credentialSubject || !credential.proof) {
        throw new BadRequestException('Invalid credential format');
      }

      // Check if the credential exists in our database
      const credentialRecord = await this.credentialModel.findOne({
        id: credential.id,
      }).exec();

      if (!credentialRecord) {
        return {
          valid: false,
          did: credential.credentialSubject.id,
          gsyDexAddress: credential.credentialSubject.accountLink?.gsyDexAddress,
          details: {
            status: 'unknown',
            reason: 'Credential not found in the system',
          },
        };
      }

      // Check if the credential is revoked
      if (credentialRecord.status === CredentialStatus.REVOKED) {
        return {
          valid: false,
          did: credentialRecord.did,
          gsyDexAddress: credentialRecord.gsyDexAddress,
          details: {
            status: 'revoked',
            reason: 'Credential has been revoked',
          },
        };
      }

      // Check expiration
      const expirationDate = new Date(credential.expirationDate);
      const now = new Date();
      if (expirationDate < now) {
        return {
          valid: false,
          did: credentialRecord.did,
          gsyDexAddress: credentialRecord.gsyDexAddress,
          details: {
            status: 'expired',
            reason: 'Credential has expired',
          },
        };
      }

      // Verify signature
      const { proof, ...credentialWithoutProof } = credential;
      const credentialString = JSON.stringify(credentialWithoutProof);
      const issuerAddress = credential.issuer.split(':')[2];
      
      try {
        const recoveredAddress = ethers.verifyMessage(credentialString, proof.jws);
        
        if (recoveredAddress.toLowerCase() !== issuerAddress.toLowerCase()) {
          return {
            valid: false,
            did: credentialRecord.did,
            gsyDexAddress: credentialRecord.gsyDexAddress,
            details: {
              status: 'invalid',
              reason: 'Invalid signature',
            },
          };
        }
      } catch (error) {
        return {
          valid: false,
          did: credentialRecord.did,
          gsyDexAddress: credentialRecord.gsyDexAddress,
          details: {
            status: 'invalid',
            reason: 'Failed to verify signature',
          },
        };
      }

      // Log the credential verification
      await this.auditService.log(
        AuditAction.CREDENTIAL_VERIFIED,
        credentialRecord.did,
        req,
        { credentialId: credential.id },
        credentialRecord.gsyDexAddress,
      );

      return {
        valid: true,
        did: credentialRecord.did,
        gsyDexAddress: credentialRecord.gsyDexAddress,
        details: {
          issuer: 'valid',
          signature: 'valid',
          expiration: 'valid',
          status: 'active',
        },
      };
    } catch (error) {
      this.logger.error(`Failed to verify credential: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new Error(`Failed to verify credential: ${error.message}`);
    }
  }

  async revokeCredential(id: string, req?: any): Promise<boolean> {
    try {
      const credentialRecord = await this.credentialModel.findOne({ id }).exec();
      if (!credentialRecord) {
        throw new BadRequestException('Credential not found');
      }

      // Update the credential status
      credentialRecord.status = CredentialStatus.REVOKED;
      await credentialRecord.save();

      // Log the revocation
      await this.auditService.log(
        AuditAction.CREDENTIAL_REVOKED,
        credentialRecord.did,
        req,
        { credentialId: id },
        credentialRecord.gsyDexAddress,
      );

      return true;
    } catch (error) {
      this.logger.error(`Failed to revoke credential: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new Error(`Failed to revoke credential: ${error.message}`);
    }
  }

  async getCredentialsByDid(did: string): Promise<Credential[]> {
    return this.credentialModel.find({ did }).exec();
  }
}