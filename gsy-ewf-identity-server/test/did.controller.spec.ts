import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import request from 'supertest';
import { DIDController } from '../src/did/did.controller';
import { DIDService } from '../src/did/did.service';
import { DIDAuthGuard } from '../src/auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../src/auth/guards/did-owner.guard';
import { ApiKeyGuard } from '../src/auth/guards/api-key.guard';
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
const mockApiKeyGuard = { canActivate: jest.fn(() => true) };

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
      .overrideGuard(ApiKeyGuard)
      .useValue(mockApiKeyGuard)
      .compile();

    controller = module.get<DIDController>(DIDController);
    didService = module.get<DIDService>(DIDService); 

    jest.clearAllMocks();
  });

  it('should be defined', () => {
    expect(controller).toBeDefined();
  });

  describe('createDID', () => {
    // The guards are overridden with permissive mocks above, so this suite can never
    // observe a real 401. Assert on the route metadata instead: POST /did was
    // unauthenticated (plan §0.5 bug A) and must stay behind ApiKeyGuard.
    // This assertion reads the decorator metadata directly and is therefore
    // independent of the .overrideGuard(...) chain.
    it('should be protected by ApiKeyGuard', () => {
      const guards = Reflect.getMetadata('__guards__', DIDController.prototype.createDID);

      expect(guards).toBeDefined();
      expect(guards).toContain(ApiKeyGuard);
    });

    it('should not rely on DIDOwnerGuard, which is permissive without a :did param', () => {
      const guards = Reflect.getMetadata('__guards__', DIDController.prototype.createDID) ?? [];

      expect(guards).not.toContain(DIDOwnerGuard);
    });

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

// Separate app, guards NOT overridden, so the real ApiKeyGuard runs over HTTP.
// This is the "401 on POST /did without the key" check from plan §7 unit 1.
describe('DIDController POST /did api-key enforcement (http)', () => {
  const API_KEY = 'test_api_key';
  let app: INestApplication;

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      controllers: [DIDController],
      providers: [
        { provide: DIDService, useValue: mockDIDService },
        {
          provide: ConfigService,
          useValue: {
            get: (key: string) => (key === 'identity.apiKey' ? API_KEY : undefined),
          },
        },
      ],
    })
      .overrideGuard(DIDAuthGuard)
      .useValue(mockDIDAuthGuard)
      .overrideGuard(DIDOwnerGuard)
      .useValue(mockDIDOwnerGuard)
      .compile();

    app = moduleFixture.createNestApplication();
    await app.init();
  });

  afterAll(async () => {
    await app.close();
  });

  beforeEach(() => {
    jest.clearAllMocks();
    mockDIDService.createDID.mockResolvedValue({
      to: '0xRegistry',
      data: '0xabcdef',
      value: '0',
    });
  });

  it('should reject a request with no x-api-key header', async () => {
    const response = await request(app.getHttpServer())
      .post('/did')
      .send({ address: '0x123' })
      .expect(401);

    expect(response.body.message).toBe('invalid or missing x-api-key');
    expect(mockDIDService.createDID).not.toHaveBeenCalled();
  });

  it('should reject a request with a wrong x-api-key header', async () => {
    await request(app.getHttpServer())
      .post('/did')
      .set('x-api-key', 'not_the_key')
      .send({ address: '0x123' })
      .expect(401);

    expect(mockDIDService.createDID).not.toHaveBeenCalled();
  });

  it('should reject a wrong-length x-api-key with 401, not 500', async () => {
    await request(app.getHttpServer())
      .post('/did')
      .set('x-api-key', 'x')
      .send({ address: '0x123' })
      .expect(401);
  });

  it('should accept a request with the correct x-api-key header', async () => {
    await request(app.getHttpServer())
      .post('/did')
      .set('x-api-key', API_KEY)
      .send({ address: '0x123' })
      .expect(200);

    expect(mockDIDService.createDID).toHaveBeenCalled();
  });
});
