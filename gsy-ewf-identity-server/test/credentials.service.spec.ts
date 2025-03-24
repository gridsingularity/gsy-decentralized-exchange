import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getModelToken } from '@nestjs/mongoose';
import { BadRequestException, UnauthorizedException } from '@nestjs/common';
import { CredentialsService } from '../src/credentials/credentials.service';
import { DIDService } from '../src/did/did.service';
import { AuditService } from '../src/audit/audit.service';
import { Credential, CredentialStatus } from '../src/database/schemas/credential.schema';
import { User } from '../src/database/schemas/user.schema';
import { AuditAction } from '../src/database/schemas';

// Mock the substrate verification utils
jest.mock('../src/credentials/utils/substrate-verification', () => ({
  verifySubstrateSignature: jest.fn().mockImplementation(async (message, signature, address) => {
    // Always return true for valid_substrate_signature in tests
    if (signature === 'valid_substrate_signature') {
      return true;
    }
    // Return false for all other signatures
    return false;
  }),
  formatSubstrateSigningMessage: jest.fn(message => `<Bytes>${message}</Bytes>`),
}));

// Mock ethers v6
jest.mock('ethers', () => ({
  verifyMessage: jest.fn((message, signature) => {
    // For valid DID signature
    if (signature === 'valid_did_signature') {
      return '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    }
    // For valid issuer signature
    if (signature === 'valid_issuer_signature') {
      return '0x1234567890123456789012345678901234567890';
    }
    // For invalid cases, return a different address
    return '0x1111111111111111111111111111111111111111';
  }),
  Wallet: class {
    // Don't store privateKey as a property since it doesn't exist on the Wallet type
    constructor(privateKey) {
      // Store as a private variable or just don't use it
      // The real Wallet class does validation, but we just need the mock methods
    }
    signMessage() {
      return Promise.resolve('valid_issuer_signature');
    }
  }
}));

// Mock Keys from @ew-did-registry/keys
jest.mock('@ew-did-registry/keys', () => ({
  Keys: jest.fn().mockImplementation(() => ({
    privateKey: '0x1234567890123456789012345678901234567890123456789012345678901234',
    publicKey: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
    getAddress: jest.fn().mockReturnValue('0x1234567890123456789012345678901234567890'),
  })),
}));

describe('CredentialsService', () => {
  let service: CredentialsService;
  let didService: DIDService;
  let substrateSigVerify: any;
  
  let mockCredentialModel: any;
  let mockUserModel: any;
  let mockAuditService: any;
  let mockConfigService: any;

  beforeEach(async () => {
    // Get the mocked substrate verification functions
    const subVerify = require('../src/credentials/utils/substrate-verification');
    substrateSigVerify = subVerify.verifySubstrateSignature;
    
    // Create mock Credential model
    mockCredentialModel = function() {
      return {
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 5000), // 5 year from now
        save: jest.fn().mockResolvedValue(true),
      };
    };
    
    mockCredentialModel.findOne = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 5000), // 5 year from now
        save: jest.fn().mockResolvedValue(true),
      }),
    });
    
    mockCredentialModel.find = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue([{
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000),
      }]),
    });
    
    // Create mock User model
    mockUserModel = function() {
      return {
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        hasVerifiedCredential: true,
        save: jest.fn().mockResolvedValue(true),
      };
    };
    
    mockUserModel.findOneAndUpdate = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        hasVerifiedCredential: true,
      }),
    });
    
    // Create mock Audit service
    mockAuditService = {
      log: jest.fn().mockResolvedValue(true),
    };
    
    // Create mock Config service
    mockConfigService = {
      get: jest.fn().mockImplementation((key) => {
        if (key === 'ewc.issuerPrivateKey') {
          return '0x1234567890123456789012345678901234567890123456789012345678901234';
        }
        if (key === 'ewc.issuerPublicKey') {
          return '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc';
        }
        return null;
      }),
    };
    
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        CredentialsService,
        {
          provide: ConfigService,
          useValue: mockConfigService,
        },
        {
          provide: DIDService,
          useValue: {
            isDIDRegistered: jest.fn().mockResolvedValue(true),
          },
        },
        {
          provide: getModelToken(Credential.name),
          useValue: mockCredentialModel,
        },
        {
          provide: getModelToken(User.name),
          useValue: mockUserModel,
        },
        {
          provide: AuditService,
          useValue: mockAuditService,
        },
      ],
    }).compile();

    service = module.get<CredentialsService>(CredentialsService);
    didService = module.get<DIDService>(DIDService);
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('issueCredential', () => {
    it('should issue a credential with valid signatures', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      const result = await service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      );
      
      expect(result).toBeDefined();
      expect(result.id).toBeDefined();
      expect(result.credential).toBeDefined();
      expect(result.credential['@context']).toContain('https://www.w3.org/2018/credentials/v1');
      expect(result.credential.credentialSubject.id).toBe(did);
      expect(result.credential.credentialSubject.accountLink.gsyDexAddress).toBe(gsyDexAddress);
      expect(mockAuditService.log).toHaveBeenCalled();
      expect(substrateSigVerify).toHaveBeenCalled();
    });

    it('should throw an error for unregistered DID', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      // Mock DID service to return false for unregistered DID
      jest.spyOn(didService, 'isDIDRegistered').mockResolvedValueOnce(false);
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(BadRequestException);
    });

    it('should throw an error for invalid DID signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'invalid_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(UnauthorizedException);
      
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_ISSUED,
        did,
        undefined,  // req parameter is undefined in test
        expect.objectContaining({ 
          gsyDexAddress,
          error: 'Invalid DID signature'
        }),
        gsyDexAddress,
        false,
      );
    });

    it('should throw an error for invalid Substrate signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'invalid_substrate_signature';
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(UnauthorizedException);
      
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_ISSUED,
        did,
        undefined,
        expect.objectContaining({ 
          gsyDexAddress,
          error: 'Invalid Substrate signature'
        }),
        gsyDexAddress,
        false,
      );
      expect(substrateSigVerify).toHaveBeenCalled();
    });
  });
  
  describe('verifyCredential', () => {
    it('should verify a valid credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date().toISOString(),
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(),
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
            chain: 'GSYDex',
          },
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date().toISOString(),
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          proofPurpose: 'assertionMethod',
          jws: 'valid_issuer_signature',
        },
      };
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(true);
      expect(result.did).toBe('did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93');
      expect(result.gsyDexAddress).toBe('5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN');
      expect(mockAuditService.log).toHaveBeenCalled();
    });

    it('should return invalid for revoked credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date().toISOString(),
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(),
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          }
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date().toISOString(),
          proofPurpose: 'assertionMethod',
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          jws: 'valid_issuer_signature'
        }
      };
      
      // Mock credentialModel.findOne to return a revoked credential
      mockCredentialModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce({
          id: 'urn:uuid:test-credential-id',
          did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          status: CredentialStatus.REVOKED,
        }),
      });
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(false);
      expect(result.details.status).toBe('revoked');
    });

    it('should return invalid for expired credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString(),
        expirationDate: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(), // Expired 1 day ago
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          }
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString(),
          proofPurpose: 'assertionMethod',
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          jws: 'valid_issuer_signature'
        }
      };
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(false);
      expect(result.details.status).toBe('expired');
    });
  });

  describe('revokeCredential', () => {
    it('should revoke a valid credential', async () => {
      const id = 'urn:uuid:test-credential-id';
      
      const result = await service.revokeCredential(id);
      
      expect(result).toBe(true);
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_REVOKED,
        expect.any(String),
        undefined,  // req parameter is undefined in test
        expect.objectContaining({ credentialId: id }),
        expect.any(String),
      );
    });

    it('should throw an error for non-existent credential', async () => {
      const id = 'urn:uuid:non-existent-id';
      
      // Mock credentialModel.findOne to return null for non-existent credential
      mockCredentialModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce(null),
      });
      
      await expect(service.revokeCredential(id)).rejects.toThrow(BadRequestException);
    });
  });

  describe('getCredentialsByDid', () => {
    it('should return credentials for a valid DID', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      
      const result = await service.getCredentialsByDid(did);
      
      expect(result).toBeDefined();
      expect(Array.isArray(result)).toBe(true);
      expect(result.length).toBeGreaterThan(0);
      expect(result[0].did).toBe(did);
    });

    it('should return empty array for DID with no credentials', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      
      // Mock credentialModel.find to return empty array
      mockCredentialModel.find.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce([]),
      });
      
      const result = await service.getCredentialsByDid(did);
      
      expect(result).toBeDefined();
      expect(Array.isArray(result)).toBe(true);
      expect(result.length).toBe(0);
    });
  });
});