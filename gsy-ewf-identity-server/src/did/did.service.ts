import { Injectable, Logger, BadRequestException, InternalServerErrorException } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { Methods } from '@ew-did-registry/did';
import { DIDDocumentFull } from '@ew-did-registry/did-document';
import { 
  EwSigner, 
  Operator 
} from '@ew-did-registry/did-ethr-resolver';
import { 
  DIDAttribute, 
  PubKeyType, 
  Encoding, 
  ProviderTypes,
  ProviderSettings 
} from '@ew-did-registry/did-resolver-interface';
import { 
  Keys, 
  KeyType 
} from '@ew-did-registry/keys';
import { DIDRequest } from './dto/did-request.dto';
import { User } from '../database/schemas';
import { AuditService } from '../audit/audit.service';
import { AuditAction } from '../database/schemas';

@Injectable()
export class DIDService {
  private readonly logger = new Logger(DIDService.name);
  private issuerKeys: Keys;
  private issuerSigner: EwSigner;
  private issuerOperator: Operator;
  private providerSettings: ProviderSettings;
  private registryAddress: string;

  constructor(
    private configService: ConfigService,
    @InjectModel(User.name) private userModel: Model<User>,
    private auditService: AuditService,
  ) {
    this.initializeProvider();
  }

  private async initializeProvider() {
    try {
      const rpcUrl = this.configService.get<string>('ewc.rpcUrl');
      const privateKey = this.configService.get<string>('ewc.issuerPrivateKey');
      const publicKey = this.configService.get<string>('ewc.issuerPublicKey');
      this.registryAddress = this.configService.get<string>('ewc.didRegistryAddress');
      
      // Initialize provider settings
      this.providerSettings = {
        type: ProviderTypes.HTTP,
        uriOrInfo: rpcUrl,
      };
      
      // Initialize Keys and EwSigner for the issuer
      this.issuerKeys = new Keys({ privateKey, publicKey });
      this.issuerSigner = EwSigner.fromPrivateKey(this.issuerKeys.privateKey, this.providerSettings);
      
      // Initialize Operator
      this.issuerOperator = new Operator(this.issuerSigner, { address: this.registryAddress });
      
      this.logger.log('DID service initialized successfully');
    } catch (error) {
      this.logger.error(`Failed to initialize DID service: ${error.message}`);
      throw new InternalServerErrorException('Failed to initialize DID service');
    }
  }

  /**
   * Check if a DID document is active on the blockchain
   * A DID is considered active if it has keys, services, or other attributes
   */
  private isActiveDocument(document: any): boolean {
    if (!document) return false;
    
    // Check if the document has any public keys
    const hasKeys = document.publicKey && Array.isArray(document.publicKey) && document.publicKey.length > 0;
    
    // Check if the document has any services
    const hasServices = document.service && Array.isArray(document.service) && document.service.length > 0;
    
    // Check if the document has authentication methods
    const hasAuth = document.authentication && Array.isArray(document.authentication) && document.authentication.length > 0;
    
    return hasKeys || hasServices || hasAuth;
  }

  async createDID(didRequest: DIDRequest, req?: any): Promise<any> {
    try {
      // Format the DID according to EWF standards
      const did = `did:${Methods.Erc1056}:${didRequest.address}`;

      // Create a DIDDocumentFull instance using the issuerOperator
      const didDocument = new DIDDocumentFull(did, this.issuerOperator);
      
      // Check if the DID already exists and is active
      let existingDocument;
      try {
        existingDocument = await didDocument.read();
      } catch (error) {
        this.logger.log(`No existing DID document found: ${error.message}`);
        // This is expected for new DIDs, continue with creation
      }
      
      // If we have an active DID document, the DID already exists
      if (existingDocument && this.isActiveDocument(existingDocument)) {
        throw new BadRequestException('DID already exists');
      }

      // Create the DID document
      await didDocument.create();
      
      // Store the DID in the database
      const user = new this.userModel({
        did,
        metadata: didRequest.metadata,
      });
      await user.save();
      
      // Log the DID creation
      await this.auditService.log(
        AuditAction.DID_CREATED,
        did,
        req,
        { address: didRequest.address },
      );
      
      // Read and return the newly created DID document
      let document;
      try {
        document = await didDocument.read();
        
        // If the document is not active after creation, log a warning
        if (!this.isActiveDocument(document)) {
          this.logger.warn(`Created DID document is not active: ${did}`);
        }
      } catch (error) {
        this.logger.error(`Failed to read DID document after creation: ${error.message}`);
        // Return a simulated document to avoid undefined
        throw new BadRequestException('Failed to read DID document after creation');
      }
      
      return {
        did,
        document
      };
    } catch (error) {
      this.logger.error(`Failed to create DID: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new InternalServerErrorException(`Failed to create DID: ${error.message}`);
    }
  }

  async resolveDID(did: string): Promise<any> {
    try {
      // Validate DID format
      if (!did.startsWith(`did:${Methods.Erc1056}:`)) {
        throw new BadRequestException('Invalid DID format');
      }
      
      // Create a DIDDocumentFull instance using the issuerOperator
      const didDocument = new DIDDocumentFull(did, this.issuerOperator);
      
      // Read the DID document
      let document;
      try {
        document = await didDocument.read();
        
        // Check if the DID document is actually active
        if (!this.isActiveDocument(document)) {
          throw new BadRequestException('DID not found or inactive');
        }
      } catch (error) {
        if (error instanceof BadRequestException) {
          throw error;
        }
        this.logger.error(`Error resolving DID: ${error.message}`);
        throw new BadRequestException('DID not found');
      }
      
      return document;
    } catch (error) {
      this.logger.error(`Failed to resolve DID: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new InternalServerErrorException(`Failed to resolve DID: ${error.message}`);
    }
  }

  async updateDID(did: string, updates: any, req?: any): Promise<any> {
    try {
      // Validate DID format
      if (!did.startsWith(`did:${Methods.Erc1056}:`)) {
        throw new BadRequestException('Invalid DID format');
      }
      
      // Create a DIDDocumentFull instance using the issuerOperator
      const didDocument = new DIDDocumentFull(did, this.issuerOperator);
      
      // Check if the DID exists and is active
      let existingDocument;
      try {
        existingDocument = await didDocument.read();
        if (!this.isActiveDocument(existingDocument)) {
          throw new BadRequestException('DID not found or inactive');
        }
      } catch (error) {
        if (error instanceof BadRequestException) {
          throw error;
        }
        throw new BadRequestException('DID not found');
      }
      
      // Handle different types of updates
      if (updates.publicKey) {
        // Add a public key to the DID document
        const validity = 365 * 24 * 60 * 60 * 1000; // 1 year validity
        await didDocument.update(
          DIDAttribute.PublicKey,
          {
            type: PubKeyType.VerificationKey2018,
            algo: KeyType.ED25519,
            encoding: Encoding.HEX,
            value: { 
              publicKey: `0x${updates.publicKey}`, 
              tag: updates.keyTag || 'key-1' 
            },
          },
          validity
        );
      }
      
      // Update user metadata if it exists
      if (updates.metadata) {
        await this.userModel.findOneAndUpdate(
          { did },
          { $set: { metadata: updates.metadata } },
          { new: true },
        );
      }
      
      // Log the DID update
      await this.auditService.log(
        AuditAction.DID_UPDATED,
        did,
        req,
        updates,
      );
      
      // Read and return the updated DID document
      let updatedDocument;
      try {
        updatedDocument = await didDocument.read();
        if (!this.isActiveDocument(updatedDocument)) {
          this.logger.warn(`Updated DID document is not active: ${did}`);
        }
      } catch (error) {
        this.logger.error(`Failed to read DID document after update: ${error.message}`);
        // Use the existing document as fallback to avoid undefined
        updatedDocument = existingDocument;
      }
      
      return updatedDocument;
    } catch (error) {
      this.logger.error(`Failed to update DID: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new InternalServerErrorException(`Failed to update DID: ${error.message}`);
    }
  }

  async isDIDRegistered(did: string): Promise<boolean> {
    try {
      // First check in our local database
      const user = await this.userModel.findOne({ did }).exec();
      if (user) {
        return true;
      }
      
      // If not found in database, try to resolve the DID on the blockchain
      try {
        // Create a DIDDocumentFull instance using the issuerOperator
        const didDocument = new DIDDocumentFull(did, this.issuerOperator);
        
        const document = await didDocument.read();
        return this.isActiveDocument(document);
      } catch (error) {
        // If the DID doesn't exist or isn't active, it's not registered
        return false;
      }
    } catch (error) {
      this.logger.error(`Failed to check DID registration: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new InternalServerErrorException('Failed to check DID registration');
    }
  }

  async deactivateDID(did: string, req?: any): Promise<boolean> {
    try {
      // Validate DID format
      if (!did.startsWith(`did:${Methods.Erc1056}:`)) {
        throw new BadRequestException('Invalid DID format');
      }
      
      // Create a DIDDocumentFull instance using the issuerOperator
      const didDocument = new DIDDocumentFull(did, this.issuerOperator);
      
      // Check if the DID exists and is active
      try {
        const existingDocument = await didDocument.read();
        if (!this.isActiveDocument(existingDocument)) {
          throw new BadRequestException('DID not found or inactive');
        }
      } catch (error) {
        if (error instanceof BadRequestException) {
          throw error;
        }
        throw new BadRequestException('DID not found');
      }
      
      // Deactivate the DID
      await didDocument.deactivate();
      
      // Update the user record
      await this.userModel.findOneAndUpdate(
        { did },
        { $set: { deactivated: true } },
        { new: true },
      );
      
      // Log the DID deactivation
      await this.auditService.log(
        AuditAction.DID_UPDATED,
        did,
        req,
        { deactivated: true },
      );
      
      return true;
    } catch (error) {
      this.logger.error(`Failed to deactivate DID: ${error.message}`);
      if (error instanceof BadRequestException) {
        throw error;
      }
      throw new InternalServerErrorException(`Failed to deactivate DID: ${error.message}`);
    }
  }
}