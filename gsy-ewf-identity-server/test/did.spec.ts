import { ZeroAddress, Wallet, Interface as EthersInterface, encodeBytes32String, decodeBytes32String, Log, LogDescription } from 'ethers';
import { AuditAction } from '../src/database/schemas';
import { ProviderTypes } from '@ew-did-registry/did-resolver-interface';
import { Keys } from '@ew-did-registry/keys';
import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getModelToken } from '@nestjs/mongoose';
import { DIDService } from '../src/did/did.service';
import { AuditService } from '../src/audit/audit.service';
import { User } from '../src/database/schemas';
import { Methods } from '@ew-did-registry/did';
import { BadRequestException, InternalServerErrorException, NotFoundException } from '@nestjs/common';
import { PreparedTransactionDto } from '../src/did/dto/prepared-transaction.dto';
import { DIDUpdateRequest } from '../src/did/dto/did-update-request.dto';
import { ETHEREUM_DID_REGISTRY_ABI } from '../src/common/abi/EthereumDIDRegistry.abi';
import ethers from 'ethers';

jest.mock('@ew-did-registry/did-document');
jest.mock('@ew-did-registry/did-ethr-resolver');
jest.mock('@ew-did-registry/did');
jest.mock('@ew-did-registry/keys', () => ({
  Keys: jest.fn().mockImplementation(() => ({
    privateKey: '0x1234567890123456789012345678901234567890123456789012345678901234',
    publicKey: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
    getAddress: jest.fn().mockReturnValue('0xIssuerAddressMock')
  })),
}));

const generateRandomAddress = () => Wallet.createRandom().address;
const DEFAULT_VERIFICATION_KEY_ATTR_NAME = 'did/pub/Secp256k1/veriKey/hex';

describe('DIDService', () => {
  let service: DIDService;
  let mockUserModel: any;
  let mockDidRegistryContract: any;
  let mockAuditService: any;
  let mockProvider: any;

  const validAddressForCreate = generateRandomAddress().toLowerCase();
  const validAddressForUpdate = generateRandomAddress().toLowerCase();
  const validAddressForDeactivate = generateRandomAddress().toLowerCase();
  const validAddressForExists = generateRandomAddress().toLowerCase();
  const validAddressForResolve = generateRandomAddress().toLowerCase();
  const validAddressForNotFound = generateRandomAddress().toLowerCase();

  const mockUserForUpdate = { did: `did:ethr:${validAddressForUpdate}`, metadata: { old: 'data' }, toObject: () => ({ did: `did:ethr:${validAddressForUpdate}`, metadata: { old: 'data' } }), lean: jest.fn().mockReturnThis(), exec: jest.fn() };
  const mockUserForDeactivate = { did: `did:ethr:${validAddressForDeactivate}`, metadata: { old: 'data' }, toObject: () => ({ did: `did:ethr:${validAddressForDeactivate}`, metadata: { old: 'data' } }), lean: jest.fn().mockReturnThis(), exec: jest.fn() };
  const mockUserForExists = { did: `did:ethr:${validAddressForExists}`, metadata: {}, toObject: () => ({ did: `did:ethr:${validAddressForExists}`, metadata: {} }), lean: jest.fn().mockReturnThis(), exec: jest.fn() };

  const mockConfigService = {
    get: jest.fn((key) => {
        if (key === 'ewc.rpcUrl') return 'mock-rpc-url';
        if (key === 'ewc.issuerPrivateKey') return '0x1234567890123456789012345678901234567890123456789012345678901234';
        if (key === 'ewc.issuerPublicKey') return '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc';
        if (key === 'ewc.didRegistryAddress') return '0xRegistryAddressMock';
        return null;
    }),
  };

  beforeEach(async () => {
    jest.clearAllMocks();

    mockAuditService = { log: jest.fn().mockResolvedValue(true) };

    mockUserModel = jest.fn().mockImplementation(() => ({ save: jest.fn().mockResolvedValue({}) }));
    mockUserModel.findOne = jest.fn().mockReturnValue({ exec: jest.fn().mockResolvedValue(null), lean: jest.fn().mockReturnThis() });
    mockUserModel.findOneAndUpdate = jest.fn().mockReturnValue({ exec: jest.fn().mockResolvedValue(null) });
    mockUserModel.updateOne = jest.fn().mockReturnValue({ exec: jest.fn().mockResolvedValue({ matchedCount: 1, modifiedCount: 1 }) });

    mockProvider = {
        getNetwork: jest.fn().mockResolvedValue({ chainId: BigInt(73799), name: 'volta' })
    };

    mockDidRegistryContract = {
        identityOwner: jest.fn().mockResolvedValue(ZeroAddress),
        changed: jest.fn().mockResolvedValue(BigInt(0)),
        queryFilter: jest.fn().mockResolvedValue([]),
        filters: {
            DIDAttributeChanged: jest.fn().mockReturnValue({ topicHash: '0xTopicHash...'}),
        },
        setAttribute: {
            populateTransaction: jest.fn().mockResolvedValue({
                to: '0xRegistryAddressMock',
                data: '0xSetAttributeDataMocked',
                value: BigInt(0)
            })
        },
        changeOwner: {
            populateTransaction: jest.fn().mockResolvedValue({
                to: '0xRegistryAddressMock',
                data: '0xChangeOwnerDataMocked',
                value: BigInt(0)
            })
        },
        interface: {
             parseLog: jest.fn().mockReturnValue(null)
        }
    };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        DIDService,
        { provide: getModelToken(User.name), useValue: mockUserModel },
        { provide: ConfigService, useValue: mockConfigService },
        { provide: AuditService, useValue: mockAuditService },
      ],
    }).compile();

    service = module.get<DIDService>(DIDService);

    (service as any).issuerKeys = new Keys({ privateKey: '0x...', publicKey: '0x...' });
    (service as any).provider = mockProvider;
    (service as any).didRegistryContract = mockDidRegistryContract;
    (service as any).registryAddress = '0xRegistryAddressMock';
    (service as any).logger = { log: jest.fn(), error: jest.fn(), warn: jest.fn(), debug: jest.fn() };

  });

  it('should be defined', () => {
     expect(service).toBeDefined();
     expect((service as any).didRegistryContract).toBeDefined();
  });

  describe('createDID', () => {
    it('should prepare setAttribute transaction and save local record', async () => {
        const address = validAddressForCreate;
        const did = `did:ethr:${address}`;
        const metadata = { name: 'Test Create' };

        mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(null) });
        mockDidRegistryContract.identityOwner.mockResolvedValueOnce(address);
        const saveMock = jest.fn().mockResolvedValue({ did, metadata });
        mockUserModel.mockImplementationOnce(() => ({ did, metadata, save: saveMock }));

        const result = await service.createDID({ address, metadata });

        expect(mockUserModel.findOne).toHaveBeenCalledWith({ did });
        expect(mockDidRegistryContract.identityOwner).toHaveBeenCalledWith(address);
        expect(mockDidRegistryContract.setAttribute.populateTransaction).toHaveBeenCalledWith(
            address,
            encodeBytes32String(DEFAULT_VERIFICATION_KEY_ATTR_NAME),
            address,
            BigInt(365 * 24 * 60 * 60)
        );
        expect(saveMock).toHaveBeenCalled();
        expect(mockAuditService.log).toHaveBeenCalled();
        expect(result.data).toEqual('0xSetAttributeDataMocked');
    });

     it('should throw BadRequest when DID record already exists locally', async () => {
        const address = validAddressForExists;
        const did = `did:ethr:${address}`;
        mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(mockUserForExists) });

        await expect(service.createDID({ address, metadata: {} }))
            .rejects.toThrow(new BadRequestException('DID record already exists locally'));
        expect(mockDidRegistryContract.identityOwner).not.toHaveBeenCalled();
     });

     it('should throw InternalServerError if owner state is unexpected', async () => {
        const address = validAddressForCreate;
        const did = `did:ethr:${address}`;
        mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(null) });
        mockDidRegistryContract.identityOwner.mockResolvedValueOnce(ZeroAddress);

        await expect(service.createDID({ address, metadata: {} }))
            .rejects.toThrow(InternalServerErrorException);
        expect(mockDidRegistryContract.identityOwner).toHaveBeenCalledWith(address);
     });
  });


  describe('prepareUpdateTransaction', () => {
    const did = `did:ethr:${validAddressForUpdate}`;
    const identity = validAddressForUpdate;
    const newPublicKey = Wallet.createRandom().publicKey;
    const updates: DIDUpdateRequest = { publicKey: newPublicKey, metadata: { updated: true } };
    const mockReq = { user: { did } };

    beforeEach(() => {
       mockUserModel.findOne.mockReturnValue({ exec: jest.fn().mockResolvedValue(mockUserForUpdate) });
       mockDidRegistryContract.identityOwner.mockResolvedValue(identity);
    });

    it('should prepare setAttribute transaction data', async () => {
        const result = await service.prepareUpdateTransaction(did, updates, mockReq);

        expect(mockUserModel.findOne).toHaveBeenCalledWith({ did });
        expect(mockDidRegistryContract.setAttribute.populateTransaction).toHaveBeenCalled();
        expect(mockUserModel.updateOne).toHaveBeenCalledWith({ did }, { $set: { metadata: updates.metadata } });
        expect(mockAuditService.log).toHaveBeenCalled();
        expect(result.data).toEqual('0xSetAttributeDataMocked');
    });

     it('should throw NotFound if user record does not exist locally', async () => {
         mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(null) });
         await expect(service.prepareUpdateTransaction(did, updates, mockReq))
            .rejects.toThrow(NotFoundException);
     });

     it('should throw BadRequest if authenticated user does not match DID', async () => {
         const wrongUserReq = { user: { did: 'did:ethr:0xDifferentUser...' } };
         mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(mockUserForUpdate) });
         await expect(service.prepareUpdateTransaction(did, updates, wrongUserReq))
            .rejects.toThrow(BadRequestException);
     });

     it('should throw BadRequest if no publicKey is provided', async () => {
         const metadataOnlyUpdate: DIDUpdateRequest = { metadata: { foo: 'bar' } };
         mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(mockUserForUpdate) });
         await expect(service.prepareUpdateTransaction(did, metadataOnlyUpdate, mockReq))
             .rejects.toThrow(new BadRequestException('Only publicKey updates supported currently.'));
     });
  });


  describe('prepareDeactivateTransaction', () => {
     const did = `did:ethr:${validAddressForDeactivate}`;
     const identity = validAddressForDeactivate;
     const mockReq = { user: { did } };

     beforeEach(() => {
         mockUserModel.findOne.mockReturnValue({ exec: jest.fn().mockResolvedValue(mockUserForDeactivate) });
     });

     it('should prepare changeOwner transaction data to ZeroAddress', async () => {
         const result = await service.prepareDeactivateTransaction(did, mockReq);

         expect(mockUserModel.findOne).toHaveBeenCalledWith({ did });
         expect(mockDidRegistryContract.changeOwner.populateTransaction).toHaveBeenCalledWith(identity, ZeroAddress);
         expect(mockUserModel.updateOne).toHaveBeenCalled();
         expect(mockAuditService.log).toHaveBeenCalled();
         expect(result.data).toEqual('0xChangeOwnerDataMocked');
     });

     it('should throw NotFound if user record does not exist locally', async () => {
        mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(null) });
        await expect(service.prepareDeactivateTransaction(did, mockReq))
           .rejects.toThrow(NotFoundException);
    });

    it('should throw BadRequest if authenticated user does not match DID', async () => {
        const wrongUserReq = { user: { did: 'did:ethr:0xDifferentUser...' } };
        mockUserModel.findOne.mockReturnValueOnce({ exec: jest.fn().mockResolvedValue(mockUserForDeactivate) });
        await expect(service.prepareDeactivateTransaction(did, wrongUserReq))
           .rejects.toThrow(BadRequestException);
    });
  });

  describe('isDIDRegistered', () => {
     it('should return true if DID has changes on-chain', async () => {
         const did = `did:ethr:${validAddressForExists}`;
         const address = validAddressForExists;
         mockDidRegistryContract.identityOwner.mockResolvedValueOnce(address);
         mockDidRegistryContract.changed.mockResolvedValueOnce(BigInt(12345));

         const result = await service.isDIDRegistered(did);
         expect(result).toBe(true);
         expect(mockDidRegistryContract.changed).toHaveBeenCalledWith(address);
     });

     it('should return false if DID has no changes on-chain', async () => {
         const did = `did:ethr:${validAddressForExists}`;
         const address = validAddressForExists;
         mockDidRegistryContract.identityOwner.mockResolvedValueOnce(address);
         mockDidRegistryContract.changed.mockResolvedValueOnce(BigInt(0));

         const result = await service.isDIDRegistered(did);
         expect(result).toBe(false);
         expect(mockDidRegistryContract.changed).toHaveBeenCalledWith(address);
     });

      it('should return false if owner is ZeroAddress', async () => {
          const did = `did:ethr:${validAddressForNotFound}`;
          const address = validAddressForNotFound;
          mockDidRegistryContract.identityOwner.mockResolvedValueOnce(ZeroAddress);

          const result = await service.isDIDRegistered(did);
          expect(result).toBe(false);
          expect(mockDidRegistryContract.changed).not.toHaveBeenCalled();
      });
  });

  describe('resolveDID', () => {
      const did = `did:ethr:${validAddressForResolve}`;
      const identity = validAddressForResolve;

      it('should return basic document if no attributes found', async () => {
          mockDidRegistryContract.identityOwner.mockResolvedValueOnce(identity);
          mockDidRegistryContract.changed.mockResolvedValueOnce(BigInt(0));
          mockProvider.getNetwork.mockResolvedValueOnce({ chainId: BigInt(123), name: 'testnet' });

          const result = await service.resolveDID(did);
          expect(result).toBeDefined();
          expect(result.id).toEqual(did);
          expect(result.verificationMethod).toEqual(expect.arrayContaining([expect.objectContaining({ id: `${did}#controller` })]));
          expect(mockProvider.getNetwork).toHaveBeenCalled();
      });

       it('should return document with attribute if found in logs', async () => {
          const attributeNameBytes = encodeBytes32String(DEFAULT_VERIFICATION_KEY_ATTR_NAME);
          const attributeValue = identity;
          const validTo = BigInt(Math.floor(Date.now() / 1000) + 3600);
          const blockNumber = BigInt(12345);

          mockDidRegistryContract.identityOwner.mockResolvedValueOnce(identity);
          mockDidRegistryContract.changed.mockResolvedValueOnce(blockNumber);

           const mockLogData = { topics: ['0xtopic0', `0x000000000000000000000000${identity.substring(2)}`], data: '0xdata' };
           mockDidRegistryContract.queryFilter.mockResolvedValueOnce([mockLogData]);
           mockDidRegistryContract.interface.parseLog.mockReturnValue({
                name: 'DIDAttributeChanged',
                args: { identity, name: attributeNameBytes, value: attributeValue, validTo, previousChange: BigInt(0) }
           });

           const result = await service.resolveDID(did);

           expect(mockDidRegistryContract.queryFilter).toHaveBeenCalled();
           expect(mockDidRegistryContract.interface.parseLog).toHaveBeenCalledWith(mockLogData);
           expect(result).toBeDefined();
           expect(result.verificationMethod).toEqual(expect.arrayContaining([
               expect.objectContaining({ id: `${did}#key-1`, publicKeyHex: attributeValue })
           ]));
       });

       it('should throw NotFound if owner is ZeroAddress', async () => {
          mockDidRegistryContract.identityOwner.mockResolvedValueOnce(ZeroAddress);
          await expect(service.resolveDID(did)).rejects.toThrow(NotFoundException);
          expect(mockDidRegistryContract.identityOwner).toHaveBeenCalledWith(identity);
       });
  });
});