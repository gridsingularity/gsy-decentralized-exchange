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
import { canonicalize } from '../common/canonical-json';

/**
 * The claims a `FedecomAssetCredential` asserts about an ontology subject (plan §2.7).
 *
 * `subjectUuid` is the canonical join key (`deterministic_area_uuid` / community uuid);
 * everything else is human-reconciliation context. `assetName`/`assetType` are absent on a
 * community subject, and absent means ABSENT: an `undefined` value would be rejected by
 * `canonicalize()` rather than silently dropped from the signed bytes.
 */
export interface AssetCredentialClaims {
  subjectUuid: string;
  communityName: string;
  communityUuid: string;
  assetName?: string;
  assetType?: string;
}

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

      // Sign the credential.
      // MUST be `canonicalize()` and never a bare `JSON.stringify`: `verifyCredential`
      // re-derives these exact bytes from a credential whose key order has been through
      // Mongo and HTTP, so the two sides only agree if both go through the one
      // canonicaliser (plan §0.5 bug B, §2.7).
      const credentialString = canonicalize(credential);
      
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

  /**
   * Issue a `FedecomAssetCredential`: the platform issuer attesting ABOUT an asset or a
   * community, not with it (plan §2.7).
   *
   * Three things `issueCredential` requires are deliberately absent, and none of them is
   * an oversight:
   *
   * 1. NO `isDIDRegistered` GATE. Phase 1 writes nothing on-chain; a `did:ethr` resolves to
   *    a valid default DID document with zero registry transactions (plan §2.3), so
   *    requiring registration would gate credentials on an optional, unscheduled phase.
   * 2. NO HOLDER (DID) SIGNATURE. The server holds the asset key, so it *could* sign the
   *    challenge it just issued - and that would prove nothing whatsoever. A self-issued,
   *    self-signed challenge is not evidence, and fabricating one would make the credential
   *    look better attested than it is. The issuer signature is the only real evidence here
   *    and it is the only one claimed.
   * 3. NO SUBSTRATE SIGNATURE. An asset has no Substrate account; every order today is
   *    signed by `dev::alice()`, so there is nothing meaningful to bind to.
   *
   * `gsyDexAddress` is left UNSET on the stored record (the schema does not require it)
   * rather than filled with a placeholder: an asset has no such address, and inventing one
   * would put a false linkage in the collection that `verifyCredential` reads back.
   *
   * The trust statement is therefore exactly: "the holder of the issuer key asserts that
   * DID X is the ontology subject with this uuid, name and type". Nothing more.
   */
  async issueAssetCredential(
    assetDid: string,
    claims: AssetCredentialClaims,
    req?: any,
  ): Promise<CredentialIssuanceResponse> {
    try {
      if (!assetDid || !assetDid.startsWith('did:ethr:')) {
        throw new BadRequestException('assetDid must be a did:ethr DID');
      }
      if (!claims?.subjectUuid || !claims?.communityName || !claims?.communityUuid) {
        throw new BadRequestException(
          'asset credential claims require subjectUuid, communityName and communityUuid',
        );
      }

      const id = `urn:uuid:${uuidv4()}`;
      const issuanceDate = new Date().toISOString();
      const expirationDate = new Date();
      expirationDate.setFullYear(expirationDate.getFullYear() + 1); // 1 year validity

      // Optional claims are OMITTED when absent, never set to undefined: `canonicalize`
      // rejects undefined precisely so a claim cannot vanish from the signed bytes.
      const credentialSubject: Record<string, any> = {
        id: assetDid,
        subjectUuid: claims.subjectUuid,
        communityName: claims.communityName,
        communityUuid: claims.communityUuid,
      };
      if (claims.assetName !== undefined) credentialSubject.assetName = claims.assetName;
      if (claims.assetType !== undefined) credentialSubject.assetType = claims.assetType;

      const credential = {
        '@context': [
          'https://www.w3.org/2018/credentials/v1',
        ],
        id,
        type: ['VerifiableCredential', 'FedecomAssetCredential'],
        issuer: this.issuerDid,
        issuanceDate,
        expirationDate: expirationDate.toISOString(),
        credentialSubject,
      };

      // Same canonicaliser as every other signature in this service (plan §2.7).
      const credentialString = canonicalize(credential);

      const wallet = new ethers.Wallet(this.issuerKeys.privateKey);
      const signature = await wallet.signMessage(credentialString);

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

      // No `gsyDexAddress`, and no `users` write: an asset is not an authenticated
      // principal and must never acquire a `User` record (plan §2.5).
      const credentialRecord = new this.credentialModel({
        id,
        did: assetDid,
        credentialSubject,
        credential: credentialWithProof,
        status: CredentialStatus.ACTIVE,
        expirationDate,
      });
      await credentialRecord.save();

      await this.auditService.log(
        AuditAction.ASSET_CREDENTIAL_ISSUED,
        assetDid,
        req,
        {
          credentialId: id,
          subjectUuid: claims.subjectUuid,
          communityUuid: claims.communityUuid,
        },
      );

      return {
        id,
        credential: credentialWithProof,
      };
    } catch (error) {
      this.logger.error(`Failed to issue asset credential: ${error.message}`);
      if (error instanceof BadRequestException || error instanceof UnauthorizedException) {
        throw error;
      }
      throw new Error(`Failed to issue asset credential: ${error.message}`);
    }
  }

  async verifyCredential(
    credential: Record<string, any>,
    req?: any,
  ): Promise<CredentialVerificationResponse> {
    try {
      this.logger.debug(`Received credential type: ${typeof credential}, Keys: ${Object.keys(credential)}`);
      try {
        this.logger.debug(`Raw credential input: ${JSON.stringify(credential)}`);
      } catch (e) {
        this.logger.error(`Failed to stringify raw credential: ${e.message}`);
      }

      if (!credential || !credential.id || !credential.issuer ||
          !credential.credentialSubject || !credential.proof) {
        this.logger.warn(`Invalid credential format received.`);
        throw new BadRequestException('Invalid credential format');
      }

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

      const { proof, ...credentialWithoutProof } = credential;

      this.logger.debug(`credentialWithoutProof type: ${typeof credentialWithoutProof}, Keys: ${Object.keys(credentialWithoutProof)}`);
       try {
        this.logger.debug(`credentialWithoutProof content: ${JSON.stringify(credentialWithoutProof, null, 2)}`);
      } catch (e) {
        this.logger.error(`Failed to stringify credentialWithoutProof: ${e.message}`);
      }

      // Same canonicaliser as the issue path, deliberately. The array-replacer form that
      // used to be here applied the top-level key list at every nesting level and so
      // deleted every nested claim before verification (plan §0.5 bug B, defect 2).
      const credentialString = canonicalize(credentialWithoutProof);
      const issuerAddress = credential.issuer.split(':')[2];
      const signatureToVerify = proof.jws;

      this.logger.log(`Verifying Message String (canonical): >>>${credentialString}<<<`);
      this.logger.log(`Verifying Signature (JWS): ${signatureToVerify}`);
      this.logger.log(`Expected Issuer Addr: ${issuerAddress}`);

      try {
        const recoveredAddress = ethers.verifyMessage(credentialString, signatureToVerify);
        this.logger.log(`Recovered address: ${recoveredAddress}`);

        if (recoveredAddress.toLowerCase() !== issuerAddress.toLowerCase()) {
          this.logger.warn(`Signature INVALID - Recovered address ${recoveredAddress} !== Issuer ${issuerAddress}`);
          return {
            valid: false,
            did: credentialRecord.did,
            gsyDexAddress: credentialRecord.gsyDexAddress,
            details: {
              status: 'invalid',
              reason: 'Invalid signature',
            },
          };
        } else {
          this.logger.log(`Signature VALID for ${issuerAddress}`);
        }
      } catch (error) {
        this.logger.error(`ethers.verifyMessage threw error: ${error.message}`, error.stack);
        return {
          valid: false,
          did: credentialRecord.did,
          gsyDexAddress: credentialRecord.gsyDexAddress,
          details: {
            status: 'invalid',
            reason: `Signature verification error: ${error.message}`,
          },
        };
      }

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
       return {
           valid: false,
           did: credential?.credentialSubject?.id || 'unknown',
           gsyDexAddress: credential?.credentialSubject?.accountLink?.gsyDexAddress || 'unknown',
           details: { status: 'error', reason: `Verification failed: ${error.message}` }
       };
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

  async getCredentialById(id: string): Promise<Credential | null> {
    return this.credentialModel.findOne({ id }).exec();
  }
}