import { Test, TestingModule } from '@nestjs/testing';
import { DIDController } from '../src/did/did.controller';
import { DIDService } from '../src/did/did.service';
import { DIDAuthGuard } from '../src/auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../src/auth/guards/did-owner.guard';
import { PreparedTransactionDto } from '../src/did/dto/prepared-transaction.dto';
import { DIDUpdateRequest } from '../src/did/dto/did-update-request.dto';
import { DIDRequest } from '../src/did/dto/did-request.dto';

const mockDIDService = {
  createDID: jest.fn(), 
  resolveDID: jest.fn(),
  prepareUpdateTransaction: jest.fn(), 
  prepareDeactivateTransaction: jest.fn(), 
  isDIDRegistered: jest.fn(),
};

const mockDIDAuthGuard = { canActivate: jest.fn(() => true) };
const mockDIDOwnerGuard = { canActivate: jest.fn(() => true) };

describe('DIDController', () => {
  let controller: DIDController;
  let didService: DIDService;

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
    didService = module.get<DIDService>(DIDService); 

    jest.clearAllMocks();
  });

  it('should be defined', () => {
    expect(controller).toBeDefined();
  });

  describe('createDID', () => {
    it('should call didService.createDID and return prepared transaction data', async () => {
      const didRequest: DIDRequest = { address: '0x123', metadata: {} };
      const mockTxData: PreparedTransactionDto = { to: '0xRegistry', data: '0xabcdef', value: '0' };
      mockDIDService.createDID.mockResolvedValue(mockTxData);

      const result = await controller.createDID(didRequest, {}); 

      expect(didService.createDID).toHaveBeenCalledWith(didRequest, {});
      expect(result).toEqual(mockTxData);
    });
  });


  describe('prepareUpdateDIDTransaction', () => {
    it('should prepare an update transaction when user is authorized', async () => {
      const did = 'did:ethr:0x123';
      const updates: DIDUpdateRequest = { publicKey: '0x456' };
      const mockReq = { user: { did } };
      const mockTxData: PreparedTransactionDto = { to: '0xRegistry', data: '0x123456', value: '0' };

      mockDIDService.prepareUpdateTransaction.mockResolvedValue(mockTxData);

      const result = await controller.prepareUpdateDIDTransaction(did, updates, mockReq);
      expect(result).toEqual(mockTxData);
      expect(didService.prepareUpdateTransaction).toHaveBeenCalledWith(did, updates, mockReq);
    });
  });

  describe('prepareDeactivateDIDTransaction', () => {
    it('should prepare a deactivate transaction when user is authorized', async () => {
      const did = 'did:ethr:0x123';
      const mockReq = { user: { did } };
      const mockTxData: PreparedTransactionDto = { to: '0xRegistry', data: '0x987654', value: '0' };

      mockDIDService.prepareDeactivateTransaction.mockResolvedValue(mockTxData);

      const result = await controller.prepareDeactivateDIDTransaction(did, mockReq);
      expect(result).toEqual(mockTxData);
      expect(didService.prepareDeactivateTransaction).toHaveBeenCalledWith(did, mockReq);
    });
  });

  describe('isDIDRegistered', () => {
    it('should check if a DID is registered', async () => {
      const did = 'did:ethr:0x123';
      mockDIDService.isDIDRegistered.mockResolvedValue(true);

      const result = await controller.isDIDRegistered(did);
      expect(result).toEqual({ registered: true });
      expect(didService.isDIDRegistered).toHaveBeenCalledWith(did);
    });
  });

  describe('resolveDID', () => {
    it('should resolve a DID document', async () => {
      const did = 'did:ethr:0x123';
      const mockDocument = { id: did, verificationMethod: [] }; 
      mockDIDService.resolveDID.mockResolvedValue(mockDocument);

      const result = await controller.resolveDID(did);
      expect(result).toEqual(mockDocument);
      expect(didService.resolveDID).toHaveBeenCalledWith(did);
    });
  });
});