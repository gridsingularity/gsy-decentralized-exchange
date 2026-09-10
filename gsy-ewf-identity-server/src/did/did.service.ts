import { Injectable, Logger, BadRequestException, InternalServerErrorException, OnModuleInit, NotFoundException } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { Methods } from '@ew-did-registry/did';
import { Keys } from '@ew-did-registry/keys';
import { DIDRequest } from './dto/did-request.dto';
import { DIDUpdateRequest } from './dto/did-update-request.dto';
import { PreparedTransactionDto } from './dto/prepared-transaction.dto';
import { User } from '../database/schemas';
import { AuditService } from '../audit/audit.service';
import { AuditAction } from '../database/schemas';
import { ethers, Contract, JsonRpcProvider, ZeroAddress, encodeBytes32String, decodeBytes32String, Log } from 'ethers';
import { ETHEREUM_DID_REGISTRY_ABI } from '../common/abi/EthereumDIDRegistry.abi';

const DEFAULT_VERIFICATION_KEY_ATTR_NAME = 'did/pub/Secp256k1/veriKey/hex';

@Injectable()
export class DIDService implements OnModuleInit {
  private readonly logger = new Logger(DIDService.name);
  private issuerKeys: Keys;
  private registryAddress: string;
  private didRegistryContract: Contract;
  private provider: JsonRpcProvider;

  constructor(
    private configService: ConfigService,
    @InjectModel(User.name) private userModel: Model<User>,
    private auditService: AuditService,
  ) {}

  async onModuleInit() {
    await this.initializeProviderAndContract();
  }

  private async initializeProviderAndContract() {
    try {
       const rpcUrl = this.configService.get<string>('ewc.rpcUrl');
       const privateKey = this.configService.get<string>('ewc.issuerPrivateKey');
       const publicKey = this.configService.get<string>('ewc.issuerPublicKey');
       this.registryAddress = this.configService.get<string>('ewc.didRegistryAddress');

       if (!rpcUrl || !privateKey || !publicKey || !this.registryAddress) {
         throw new Error('Missing required EWC configuration');
       }

       this.provider = new ethers.JsonRpcProvider(rpcUrl);
       this.issuerKeys = new Keys({ privateKey, publicKey });
       this.didRegistryContract = new ethers.Contract(this.registryAddress, ETHEREUM_DID_REGISTRY_ABI, this.provider);

       this.logger.log('DID service provider and contract initialized successfully');
       const network = await this.provider.getNetwork();
       this.logger.log(`Connected to network: ${network.name} (Chain ID: ${network.chainId})`);
       await this.didRegistryContract.identityOwner(ZeroAddress);
       this.logger.log(`Registry contract query test successful.`);

    } catch (error) {
      this.logger.error(`Failed to initialize DID service provider/contract: ${error.message}`, error.stack);
      throw new InternalServerErrorException(`Failed to initialize DID service infrastructure: ${error.message}`);
    }
  }

  private async getDIDOwnerOnChain(did: string): Promise<string | null> {
    if (!did || !did.startsWith('did:ethr:')) { throw new Error(`Invalid DID format: ${did}`); }
    const address = did.substring(9);
    if (!ethers.isAddress(address)) { throw new Error(`Invalid Ethereum address in DID: ${address}`); }
    try {
        if (!this.didRegistryContract) { throw new Error("Registry contract not initialized."); }
        const owner = await this.didRegistryContract.identityOwner(address);
        return owner === ZeroAddress ? null : owner;
    } catch (error) {
        this.logger.error(`Error querying identityOwner for ${address}: ${error.message}`, error.stack);
        throw error;
    }
  }

  private async checkUserExistsLocally(did: string): Promise<boolean> {
     const user = await this.userModel.findOne({ did }).exec();
     return !!user;
  }

  async createDID(didRequest: DIDRequest, req?: any): Promise<PreparedTransactionDto> {
    const targetAddress = didRequest.address.toLowerCase();
    const did = `did:${Methods.Erc1056}:${targetAddress}`;
    this.logger.log(`Preparing initial transaction for DID: ${did}`);

    if (!this.didRegistryContract) {
      throw new InternalServerErrorException("DID Service not properly initialized.");
    }

    try {
      const userExists = await this.checkUserExistsLocally(did);
      if (userExists) {
        throw new BadRequestException('DID record already exists locally');
      }

      const ownerOnChain = await this.getDIDOwnerOnChain(did);
       if (ownerOnChain === null || ownerOnChain.toLowerCase() !== targetAddress) {
          throw new InternalServerErrorException(`Unexpected initial on-chain owner state for ${did}. Owner: ${ownerOnChain}`);
       }
       this.logger.log(`DID ${did} confirmed self-owned on-chain. Preparing initial attribute TX.`);

      const identity = targetAddress;
      const name = encodeBytes32String(DEFAULT_VERIFICATION_KEY_ATTR_NAME);
      const value = targetAddress;
      const validity = BigInt(365 * 24 * 60 * 60);

      const txRequest = await this.didRegistryContract.setAttribute.populateTransaction(
         identity, name, value, validity
      );

      const user = new this.userModel({ did, metadata: didRequest.metadata });
      await user.save();
      this.logger.log(`DID ${did} record saved to local database.`);
      await this.auditService.log(
        AuditAction.DID_CREATED, did, req,
        { address: didRequest.address, action: "Prepared setAttribute TX" },
      );

      return {
        to: txRequest.to!,
        data: txRequest.data!,
        value: txRequest.value?.toString() ?? '0'
      };

    } catch (error) {
      this.logger.error(`Failed to prepare createDID transaction for ${did}: ${error.message}`, error.stack);
      if (error instanceof BadRequestException || error instanceof InternalServerErrorException) { throw error; }
      throw new InternalServerErrorException(`Failed to prepare DID creation transaction: ${error.message}`);
    }
  }

  async prepareUpdateTransaction(did: string, updates: DIDUpdateRequest, req?: any): Promise<PreparedTransactionDto> {
      this.logger.log(`Preparing update transaction for DID: ${did}`);
      const userExists = await this.checkUserExistsLocally(did);
      if (!userExists) {
          throw new NotFoundException(`DID record not found locally: ${did}`);
      }
      if (req?.user?.did !== did) throw new BadRequestException('Auth mismatch');

      if (!updates.publicKey) {
          throw new BadRequestException('Only publicKey updates supported currently.');
      }
      try {
          const identity = did.substring(9);
          const name = encodeBytes32String(DEFAULT_VERIFICATION_KEY_ATTR_NAME);
          const value = updates.publicKey.startsWith('0x') ? updates.publicKey : `0x${updates.publicKey}`;
          const validity = BigInt(365 * 24 * 60 * 60);

          const txRequest = await this.didRegistryContract.setAttribute.populateTransaction(
             identity, name, value, validity
          );

          if (updates.metadata) {
             await this.userModel.updateOne({ did }, { $set: { metadata: updates.metadata } }).exec();
          }
          await this.auditService.log(AuditAction.DID_UPDATED, did, req, { action: "Prepared setAttribute TX", name: DEFAULT_VERIFICATION_KEY_ATTR_NAME, value });

          return {
             to: txRequest.to!,
             data: txRequest.data!,
             value: txRequest.value?.toString() ?? '0'
          };
      } catch (error) {
         this.logger.error(`Failed to prepare update transaction for ${did}: ${error.message}`, error.stack);
         throw new InternalServerErrorException(`Failed to prepare update transaction: ${error.message}`);
      }
  }

  async prepareDeactivateTransaction(did: string, req?: any): Promise<PreparedTransactionDto> {
       this.logger.log(`Preparing deactivation transaction for DID: ${did}`);
       const userExists = await this.checkUserExistsLocally(did);
       if (!userExists) {
           throw new NotFoundException(`DID record not found locally: ${did}`);
       }
       if (req?.user?.did !== did) throw new BadRequestException('Auth mismatch');

       try {
          const identity = did.substring(9);
          const newOwner = ZeroAddress;

          const txRequest = await this.didRegistryContract.changeOwner.populateTransaction(
              identity, newOwner
          );

          const userDoc = await this.userModel.findOne({did}).exec();
          const updatedMetadata = { ...userDoc?.metadata, deactivated: true };
          await this.userModel.updateOne({ did }, { $set: { metadata: updatedMetadata } }).exec();
          this.logger.log(`Optimistically marked local record as deactivated for DID ${did}.`);

          await this.auditService.log(AuditAction.DID_UPDATED, did, req, { action: "Prepared changeOwner TX (deactivate)", newOwner });

          return {
            to: txRequest.to!,
            data: txRequest.data!,
            value: txRequest.value?.toString() ?? '0'
          };
       } catch (error) {
          this.logger.error(`Failed to prepare deactivate transaction for ${did}: ${error.message}`, error.stack);
          throw new InternalServerErrorException(`Failed to prepare deactivate transaction: ${error.message}`);
       }
  }

  async isDIDRegistered(did: string): Promise<boolean> {
      this.logger.debug(`Checking registration status for DID: ${did}`);
      if (!this.didRegistryContract) { throw new InternalServerErrorException("DID Service infrastructure not ready."); }
      try {
          const ownerOnChain = await this.getDIDOwnerOnChain(did);
          if (!ownerOnChain) return false;

          const lastChangeBlock = await this.didRegistryContract.changed(did.substring(9));
          const registered = lastChangeBlock > 0;
          this.logger.debug(`On-chain check for ${did}: Owner = ${ownerOnChain}, LastChangeBlock = ${lastChangeBlock}, Registered = ${registered}`);
          return registered;
      } catch (error) {
          this.logger.error(`Failed to check DID registration for ${did}: ${error.message}`, error.stack);
          throw new InternalServerErrorException(`Failed to check registration status for ${did}: ${error.message}`);
      }
  }

  async resolveDID(did: string): Promise<any> {
      this.logger.debug(`Resolving DID: ${did}`);
      if (!did || !did.startsWith('did:ethr:')) { throw new BadRequestException('Invalid DID format'); }
      const identity = did.substring(9).toLowerCase();
      if (!ethers.isAddress(identity)) { throw new BadRequestException(`Invalid Ethereum address in DID: ${identity}`); }
      if (!this.didRegistryContract) { throw new InternalServerErrorException("DID Service not properly initialized."); }

      try {
          const ownerOnChain = await this.getDIDOwnerOnChain(did);
          if (!ownerOnChain || ownerOnChain.toLowerCase() !== identity) {
              throw new NotFoundException('DID not found or not self-owned');
          }

          const didDocument: any = {
              '@context': 'https://www.w3.org/ns/did/v1',
              id: did,
              verificationMethod: [],
              authentication: [],
              assertionMethod: [],
          };

          const changedBlock = Number(await this.didRegistryContract.changed(identity));
          let keyAdded = false; // Track if default key was added

          if (changedBlock > 0) {
              const attributeEvent = this.didRegistryContract.filters.DIDAttributeChanged(identity);
              const startBlock = Math.max(0, changedBlock - 10);
              const logs = await this.didRegistryContract.queryFilter(attributeEvent, startBlock);
              this.logger.debug(`Found ${logs.length} attribute logs for ${did} since block ${startBlock}`);


              for (const log of logs) {
                   // Use contract object's interface to parse log
                   const parsedLog = this.didRegistryContract.interface.parseLog({ topics: log.topics as string[], data: log.data });

                   if (parsedLog && parsedLog.name === 'DIDAttributeChanged') {
                      const nameBytes = parsedLog.args.name;
                      const value = parsedLog.args.value;
                      const validTo = Number(parsedLog.args.validTo);
                      if (validTo === 0 || validTo * 1000 > Date.now()) {
                         try {
                             const name = decodeBytes32String(nameBytes);
                             if (name === DEFAULT_VERIFICATION_KEY_ATTR_NAME) {
                                 const vmId = `${did}#key-1`; // Use a consistent ID convention
                                 didDocument.verificationMethod.push({
                                     id: vmId,
                                     type: 'EcdsaSecp256k1VerificationKey2019',
                                     controller: did,
                                     publicKeyHex: value
                                 });
                                 if (!didDocument.authentication.includes(vmId)) didDocument.authentication.push(vmId);
                                 if (!didDocument.assertionMethod.includes(vmId)) didDocument.assertionMethod.push(vmId);
                                 keyAdded = true;
                             }
                         } catch (decodeError) { this.logger.warn(`Could not decode attribute name bytes: ${nameBytes}`); }
                      }
                   }
              }
          }

          // Add controller info if no specific key was found or no changes ever occurred
          if (!keyAdded) {
              this.logger.debug(`No verification key attribute found or no changes for ${did}. Adding default controller entry.`);
               const network = await this.provider.getNetwork();
               const chainId = network.chainId;
               didDocument.verificationMethod.push({ id: `${did}#controller`, type: 'EcdsaSecp256k1RecoveryMethod2020', controller: did, blockchainAccountId: `${ownerOnChain}@eip155:${chainId}` });
               didDocument.authentication = [`${did}#controller`];
               didDocument.assertionMethod = [`${did}#controller`];
          }

          return didDocument;

      } catch (error) {
          this.logger.error(`Failed to resolve DID ${did}: ${error.message}`, error.stack);
          if (error instanceof BadRequestException || error instanceof InternalServerErrorException || error instanceof NotFoundException) {
              throw error;
          }
          throw new InternalServerErrorException(`Failed to resolve DID: ${error.message}`);
      }
  }
}