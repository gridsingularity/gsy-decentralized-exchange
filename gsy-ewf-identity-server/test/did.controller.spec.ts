import { Test, TestingModule } from '@nestjs/testing';
import { DIDController } from '../src/did/did.controller';
import { DIDService } from '../src/did/did.service';
import { DIDAuthGuard } from '../src/auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../src/auth/guards/did-owner.guard';

// Mock the DID service
const mockDIDService = {
  createDID: jest.fn(),
  resolveDID: jest.fn(),
  updateDID: jest.fn(),
  deactivateDID: jest.fn(),
  isDIDRegistered: jest.fn(),
  getUserByDid: jest.fn(),
};

// Mock the DIDAuthGuard
const mockDIDAuthGuard = { canActivate: jest.fn(() => true) };

// Mock the DIDOwnerGuard
const mockDIDOwnerGuard = { canActivate: jest.fn(() => true) };

describe('DIDController', () => {
  let controller: DIDController;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [DIDController],
      providers: [
        { provide: DIDService, useValue: mockDIDService },
      ],
    })
      .overrideGuard(DIDAuthGuard)
      .useValue(mockDIDAuthGuard)
      .overrideGuard(DIDOwnerGuard)
      .useValue(mockDIDOwnerGuard)
      .compile();

    controller = module.get<DIDController>(DIDController);

    // Reset mock function calls before each test
    jest.clearAllMocks();
  });

  it('should be defined', () => {
    expect(controller).toBeDefined();
  });

  describe('updateDID', () => {
    it('should update a DID when user is authorized', async () => {
      const did = 'did:ethr:0x123';
      const updates = { metadata: { name: 'Updated Name' } };
      const mockReq = { user: { did } };
      const mockUpdatedDoc = { id: did, metadata: updates.metadata };
      
      mockDIDOwnerGuard.canActivate.mockReturnValue(true);
      mockDIDService.updateDID.mockResolvedValue(mockUpdatedDoc);

      const result = await controller.updateDID(did, updates, mockReq);
      expect(result).toEqual(mockUpdatedDoc);
      expect(mockDIDService.updateDID).toHaveBeenCalledWith(did, updates, mockReq);
    });

    // The actual guard denial is tested in the guard's unit tests
  });

  describe('deactivateDID', () => {
    it('should deactivate a DID when user is authorized', async () => {
      const did = 'did:ethr:0x123';
      const mockReq = { user: { did } };
      
      mockDIDOwnerGuard.canActivate.mockReturnValue(true);
      mockDIDService.deactivateDID.mockResolvedValue(true);

      const result = await controller.deactivateDID(did, mockReq);
      expect(result).toEqual({ success: true });
      expect(mockDIDService.deactivateDID).toHaveBeenCalledWith(did, mockReq);
    });
  });

  describe('isDIDRegistered', () => {
    it('should check if a DID is registered', async () => {
      const did = 'did:ethr:0x123';
      
      mockDIDService.isDIDRegistered.mockResolvedValue(true);

      const result = await controller.isDIDRegistered(did);
      expect(result).toEqual({ registered: true });
      expect(mockDIDService.isDIDRegistered).toHaveBeenCalledWith(did);
    });
  });

  describe('resolveDID', () => {
    it('should resolve a DID document', async () => {
      const did = 'did:ethr:0x123';
      const mockDocument = { id: did, publicKey: [] };
      
      mockDIDService.resolveDID.mockResolvedValue(mockDocument);

      const result = await controller.resolveDID(did);
      expect(result).toEqual(mockDocument);
      expect(mockDIDService.resolveDID).toHaveBeenCalledWith(did);
    });
  });
});