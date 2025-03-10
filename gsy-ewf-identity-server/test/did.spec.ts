jest.mock('@ew-did-registry/did-document', () => {
  // Store mock implementations for each instance
  const mockInstances = new Map();
  
  return {
    DIDDocumentFull: jest.fn().mockImplementation((did) => {
      // Create a new mock instance if it doesn't exist for this DID
      if (!mockInstances.has(did)) {
        const mockInstance = {
          create: jest.fn().mockResolvedValue(true),
          read: jest.fn(),
          update: jest.fn().mockResolvedValue(true),
          deactivate: jest.fn().mockResolvedValue(true)
        };
        mockInstances.set(did, mockInstance);
      }
      
      // Return the mock instance for this DID
      return mockInstances.get(did);
    })
  };
});

jest.mock('@ew-did-registry/did-ethr-resolver', () => ({
  EwSigner: {
    fromPrivateKey: jest.fn().mockReturnValue({})
  },
  Operator: jest.fn().mockImplementation(() => ({}))
}));

jest.mock('@ew-did-registry/did', () => ({
  Methods: {
    Erc1056: 'ethr'
  }
}));

jest.mock('@ew-did-registry/keys', () => ({
  Keys: jest.fn().mockImplementation(() => ({
    privateKey: '0x1234567890123456789012345678901234567890123456789012345678901234',
    publicKey: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
    getAddress: jest.fn().mockReturnValue('0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93')
  })),
  KeyType: {
    ED25519: 'Ed25519VerificationKey'
  }
}));

import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getModelToken } from '@nestjs/mongoose';
import { DIDService } from '../src/did/did.service';
import { AuditService } from '../src/audit/audit.service';
import { User } from '../src/database/schemas';
import { Methods } from '@ew-did-registry/did';
import { BadRequestException, InternalServerErrorException } from '@nestjs/common';

describe('DIDService', () => {
  let service: DIDService;
  let mockUserModel: any;

  const mockSavedUser = {
    did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
    metadata: { name: 'Test User' },
    save: jest.fn().mockResolvedValue(true)
  };

  // Create an active DID document mock that will pass isActiveDocument check
  const createActiveDIDDocument = (did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93') => ({
    id: did,
    // Need to include this to pass isActiveDocument check
    publicKey: [{ id: 'key-1', type: 'Secp256k1VerificationKey2018' }],
    // Add these for completeness
    authentication: [{ id: 'auth-1', type: 'Secp256k1SignatureAuthentication2018' }],
    service: []
  });

  // Create a minimal DID document that will fail isActiveDocument check
  const createMinimalDIDDocument = (did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93') => ({
    id: did,
    // These empty arrays will cause isActiveDocument to return false
    publicKey: [],
    authentication: [],
    service: []
  });

  const mockConfigService = {
    get: jest.fn((key) => {
      if (key === 'ewc.rpcUrl') return 'https://volta-rpc.energyweb.org';
      if (key === 'ewc.issuerPrivateKey') return '0x1234567890123456789012345678901234567890123456789012345678901234';
      if (key === 'ewc.issuerPublicKey') return '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc';
      if (key === 'ewc.didRegistryAddress') return '0xc15d5a57a8eb0e1dcbe5d88b8f9a82017e5cc4af';
      return null;
    }),
  };

  const mockAuditService = {
    log: jest.fn().mockResolvedValue(true),
  };

  beforeEach(async () => {
    // Reset all mocks
    jest.clearAllMocks();

    // Create a new mock User model for each test
    mockUserModel = function() {
      return {
        did: '',
        metadata: {},
        save: jest.fn().mockResolvedValue(mockSavedUser)
      };
    };
    
    // Add static methods to the model
    mockUserModel.findOne = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue(null)
    });
    
    mockUserModel.findOneAndUpdate = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue(mockSavedUser)
    });
    
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        DIDService,
        {
          provide: getModelToken(User.name),
          useValue: mockUserModel
        },
        {
          provide: ConfigService,
          useValue: mockConfigService,
        },
        {
          provide: AuditService,
          useValue: mockAuditService,
        },
      ],
    }).compile();

    service = module.get<DIDService>(DIDService);
    
    jest.spyOn(service as any, 'isActiveDocument').mockImplementation((doc: any) => {
      if (!doc) return false;
      
      // Match the actual implementation logic but ensure it works with our test documents
      const hasKeys = doc.publicKey && Array.isArray(doc.publicKey) && doc.publicKey.length > 0;
      const hasServices = doc.service && Array.isArray(doc.service) && doc.service.length > 0;
      const hasAuth = doc.authentication && Array.isArray(doc.authentication) && doc.authentication.length > 0;
      
      return hasKeys || hasServices || hasAuth;
    });
    
    // Mock initializeProvider to avoid real initialization
    jest.spyOn(service as any, 'initializeProvider').mockImplementation(() => {
      (service as any).issuerKeys = {
        privateKey: '0x1234567890123456789012345678901234567890123456789012345678901234',
        publicKey: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
        getAddress: jest.fn().mockReturnValue('0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93')
      };
      (service as any).issuerSigner = {};
      (service as any).issuerOperator = {};
      (service as any).providerSettings = { type: 'HTTP', uriOrInfo: 'https://volta-rpc.energyweb.org' };
      (service as any).registryAddress = '0xc15d5a57a8eb0e1dcbe5d88b8f9a82017e5cc4af';
    });
    await (service as any).initializeProvider();
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  // Testing successful DID creation
  it('should create a DID document', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Create an active document
    const mockDocument = createActiveDIDDocument(did);
    
    // Create the DIDDocumentFull instance we'll use
    const didInstance = DIDDocumentFull(did);
    
    // First call - checking if DID exists - throw an error to simulate non-existence
    didInstance.read.mockRejectedValueOnce(new Error('DID document not found'));
    
    // Second call - after creation (should return active document)
    didInstance.read.mockResolvedValueOnce(mockDocument);
    
    // Call the method to be tested
    const result = await service.createDID({ address, metadata: { name: 'Test User' } });
    
    // Assert expectations
    expect(result).toBeDefined();
    expect(result.did).toBe(did);
    expect(result.document).toEqual(mockDocument);
    expect(didInstance.create).toHaveBeenCalled();
    expect(mockAuditService.log).toHaveBeenCalled();
  });

  // Testing DID already exists case
  it('should handle case when DID already exists', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Create the DIDDocumentFull instance we'll use
    const didInstance = DIDDocumentFull(did);
    
    // Mock the read method to return an active document (DID exists)
    const activeDocument = createActiveDIDDocument(did);
    didInstance.read.mockResolvedValueOnce(activeDocument);
    
    // Test the creation with expected error
    await expect(
      service.createDID({ address, metadata: { name: 'Test User' } })
    ).rejects.toThrow(BadRequestException);
    
    // Verify create was not called
    expect(didInstance.create).not.toHaveBeenCalled();
  });

  // Testing DID resolution
  it('should resolve a DID document', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Create the DIDDocumentFull instance we'll use
    const didInstance = DIDDocumentFull(did);
    
    // Mock return of active document
    const mockDocument = createActiveDIDDocument(did);
    didInstance.read.mockResolvedValueOnce(mockDocument);
    
    // Call the resolve method
    const result = await service.resolveDID(did);
    
    // Verify the result
    expect(result).toEqual(mockDocument);
  });

  // Testing invalid DID format
  it('should reject invalid DID format', async () => {
    const invalidDid = 'invalid-did-format';
    
    // Test with invalid DID format
    await expect(service.resolveDID(invalidDid)).rejects.toThrow(BadRequestException);
  });

  // Testing DID not found
  it('should handle DID not found', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    const didInstance = DIDDocumentFull();
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Mock read to return inactive/minimal document
    didInstance.read.mockResolvedValueOnce(createMinimalDIDDocument(did));
    
    // Test with DID that has minimal document (inactive)
    await expect(service.resolveDID(did)).rejects.toThrow(BadRequestException);
  });

  // Testing is DID registered - found in database
  it('should check if DID is registered in database', async () => {
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Set up the mock to return a user
    mockUserModel.findOne.mockReturnValueOnce({
      exec: jest.fn().mockResolvedValueOnce(mockSavedUser)
    });
    
    // Call the method to be tested
    const result = await service.isDIDRegistered(did);
    
    // Verify expectations
    expect(result).toBe(true);
    expect(mockUserModel.findOne).toHaveBeenCalledWith({ did });
  });

  // Testing is DID registered - found on blockchain
  it('should check if DID is on blockchain but not in database', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Configure findOne to return null (not in database)
    mockUserModel.findOne.mockReturnValue({
      exec: jest.fn().mockResolvedValue(null)
    });
    
    // Create the DIDDocumentFull instance we'll use
    const didInstance = DIDDocumentFull(did);
    
    // Configure read to return active document
    const activeDocument = createActiveDIDDocument(did);
    didInstance.read.mockResolvedValueOnce(activeDocument);
    
    // Call the method to be tested
    const result = await service.isDIDRegistered(did);
    
    // Verify expectations
    expect(result).toBe(true);
  });

  // Minimal test for deactivate method
  it('should handle DID deactivation', async () => {
    // Get access to the mocked DIDDocumentFull instance
    const { DIDDocumentFull } = require('@ew-did-registry/did-document');
    
    // Use address and DID consistently
    const address = '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    const did = `did:${Methods.Erc1056}:${address}`;
    
    // Create the DIDDocumentFull instance we'll use
    const didInstance = DIDDocumentFull(did);
    
    // Set up read to return active document
    const activeDocument = createActiveDIDDocument(did);
    didInstance.read.mockResolvedValueOnce(activeDocument);
    
    // Call the deactivate method
    await service.deactivateDID(did);
    
    // Verify both deactivate and database update were called
    expect(didInstance.deactivate).toHaveBeenCalled();
    expect(mockUserModel.findOneAndUpdate).toHaveBeenCalled();
  });
});