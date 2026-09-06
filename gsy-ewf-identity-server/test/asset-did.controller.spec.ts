import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication, ValidationPipe } from '@nestjs/common';
import request from 'supertest';
import { v5 as uuidv5 } from 'uuid';
import { AssetDIDController } from '../src/assets/asset-did.controller';
import { AssetDIDService } from '../src/assets/asset-did.service';
import { ApiKeyGuard } from '../src/auth/guards/api-key.guard';
import { AssetDIDSubjectType } from '../src/database/schemas';
import { AssetSyncRequest } from '../src/assets/dto/asset-sync-request.dto';

const IDENTITY_NAMESPACE = '6ba7b812-9dad-11d1-80b4-00c04fd430c8';
const communityUuid = (name: string) => uuidv5(name, IDENTITY_NAMESPACE);
const areaUuid = (community: string, asset: string) =>
  uuidv5(`${community}:${asset}`, IDENTITY_NAMESPACE);

const mockAssetDIDService = {
  syncSubjects: jest.fn(),
  findBySubjectUuid: jest.fn(),
  list: jest.fn(),
};

const mockApiKeyGuard = { canActivate: jest.fn(() => true) };

const communityItem = {
  subjectUuid: communityUuid('Pilot1'),
  communityName: 'Pilot1',
  communityUuid: communityUuid('Pilot1'),
};

const assetItem = {
  subjectUuid: areaUuid('Pilot1', 'LIC08SM'),
  assetName: 'LIC08SM',
  assetType: 'SMART_METER',
  communityName: 'Pilot1',
  communityUuid: communityUuid('Pilot1'),
  areaHash: 'ab'.repeat(32),
};

describe('AssetDIDController', () => {
  let controller: AssetDIDController;
  let app: INestApplication;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [AssetDIDController],
      providers: [{ provide: AssetDIDService, useValue: mockAssetDIDService }],
    })
      .overrideGuard(ApiKeyGuard)
      .useValue(mockApiKeyGuard)
      .compile();

    controller = module.get<AssetDIDController>(AssetDIDController);

    app = module.createNestApplication();
    // Same pipe configuration as `main.ts:10-16`. `forbidNonWhitelisted` is the whole
    // reason the rejection tests below mean anything, so it must not be relaxed here.
    app.useGlobalPipes(
      new ValidationPipe({ whitelist: true, transform: true, forbidNonWhitelisted: true }),
    );
    await app.init();

    jest.clearAllMocks();
  });

  afterEach(async () => {
    if (app) await app.close();
  });

  it('should be defined', () => {
    expect(controller).toBeDefined();
  });

  describe('guard wiring', () => {
    // The guard is overridden with a permissive mock above, so this suite can never observe
    // a real 401. Assert on the class-level decorator metadata instead - class level, so a
    // route added later is authenticated by default.
    it('is protected by ApiKeyGuard at class level', () => {
      const guards = Reflect.getMetadata('__guards__', AssetDIDController);

      expect(guards).toBeDefined();
      expect(guards).toContain(ApiKeyGuard);
    });

    it('mounts at /asset-dids, not under /did (plan §4.3, R7)', () => {
      const path = Reflect.getMetadata('path', AssetDIDController);

      // `GET /did/:did` would shadow a nested asset route depending on module registration
      // order in app.module.ts. Do not "tidy" this under /did.
      expect(path).toBe('asset-dids');
    });
  });

  describe('POST /asset-dids/sync', () => {
    it('delegates to syncSubjects with the payload and the request', async () => {
      const payload = { communities: [communityItem], assets: [assetItem] } as AssetSyncRequest;
      const response = {
        created: 2,
        updated: 0,
        retired: 0,
        subjects: [
          {
            subjectUuid: assetItem.subjectUuid,
            subjectType: AssetDIDSubjectType.ASSET,
            did: 'did:ethr:0xabc',
            registeredOnChain: false,
          },
        ],
      };
      mockAssetDIDService.syncSubjects.mockResolvedValue(response);

      const req = { ip: '127.0.0.1' };
      const result = await controller.sync(payload, req);

      expect(mockAssetDIDService.syncSubjects).toHaveBeenCalledWith(payload, req);
      expect(result).toBe(response);
    });

    it('returns 200 (not 201) for a valid payload', async () => {
      mockAssetDIDService.syncSubjects.mockResolvedValue({
        created: 1,
        updated: 0,
        retired: 0,
        subjects: [],
      });

      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [assetItem] })
        .expect(200);
    });

    it('accepts an assets-only payload and a communities-only payload', async () => {
      mockAssetDIDService.syncSubjects.mockResolvedValue({
        created: 1,
        updated: 0,
        retired: 0,
        subjects: [],
      });

      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [assetItem] })
        .expect(200);

      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ communities: [communityItem] })
        .expect(200);

      expect(mockAssetDIDService.syncSubjects).toHaveBeenCalledTimes(2);
    });

    it('rejects a payload carrying an undeclared top-level field', async () => {
      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [assetItem], surprise: 'unexpected' })
        .expect(400);

      expect(JSON.stringify(response.body)).toMatch(/surprise/);
      expect(mockAssetDIDService.syncSubjects).not.toHaveBeenCalled();
    });

    it('rejects a nested item carrying an undeclared field', async () => {
      // Without @ValidateNested + @Type this passes: class-transformer leaves nested items
      // as plain objects, and class-validator finds no metadata to enforce.
      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [{ ...assetItem, privateKey: `0x${'11'.repeat(32)}` }] })
        .expect(400);

      expect(JSON.stringify(response.body)).toMatch(/privateKey/);
      expect(mockAssetDIDService.syncSubjects).not.toHaveBeenCalled();
    });

    it('rejects a nested asset with a malformed subjectUuid', async () => {
      // The load-bearing @ValidateNested test: subjectUuid is fed straight into key
      // derivation, so an unvalidated one mints a DID for a subject that does not exist.
      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [{ ...assetItem, subjectUuid: 'not-a-uuid' }] })
        .expect(400);

      expect(JSON.stringify(response.body)).toMatch(/subjectUuid/);
      expect(mockAssetDIDService.syncSubjects).not.toHaveBeenCalled();
    });

    it('rejects a nested community with a malformed subjectUuid', async () => {
      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ communities: [{ ...communityItem, subjectUuid: '' }] })
        .expect(400);

      expect(JSON.stringify(response.body)).toMatch(/subjectUuid/);
      expect(mockAssetDIDService.syncSubjects).not.toHaveBeenCalled();
    });

    it('rejects a nested asset missing a required field', async () => {
      const { areaHash, ...withoutAreaHash } = assetItem;

      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ assets: [withoutAreaHash] })
        .expect(400);

      expect(JSON.stringify(response.body)).toMatch(/areaHash/);
    });

    it('rejects a payload with neither communities nor assets', async () => {
      await request(app.getHttpServer()).post('/asset-dids/sync').send({}).expect(400);
      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ communities: [], assets: [] })
        .expect(400);

      expect(mockAssetDIDService.syncSubjects).not.toHaveBeenCalled();
    });

    it('rejects a non-array communities field', async () => {
      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send({ communities: communityItem, assets: [assetItem] })
        .expect(400);
    });
  });

  describe('GET /asset-dids/:subjectUuid', () => {
    it('delegates to findBySubjectUuid with the path param', async () => {
      const record = { subjectUuid: assetItem.subjectUuid, did: 'did:ethr:0xabc' };
      mockAssetDIDService.findBySubjectUuid.mockResolvedValue(record);

      const result = await controller.findBySubjectUuid(assetItem.subjectUuid);

      expect(mockAssetDIDService.findBySubjectUuid).toHaveBeenCalledWith(assetItem.subjectUuid);
      expect(result).toBe(record);
    });

    it('routes the HTTP call to the single-record handler', async () => {
      mockAssetDIDService.findBySubjectUuid.mockResolvedValue({ did: 'did:ethr:0xabc' });

      await request(app.getHttpServer())
        .get(`/asset-dids/${assetItem.subjectUuid}`)
        .expect(200);

      expect(mockAssetDIDService.findBySubjectUuid).toHaveBeenCalledWith(assetItem.subjectUuid);
      expect(mockAssetDIDService.list).not.toHaveBeenCalled();
    });
  });

  describe('GET /asset-dids', () => {
    it('delegates to list with the parsed query filter', async () => {
      mockAssetDIDService.list.mockResolvedValue([]);

      await request(app.getHttpServer())
        .get('/asset-dids')
        .query({
          communityUuid: communityUuid('Pilot1'),
          subjectType: 'asset',
          registeredOnChain: 'false',
          retired: 'false',
        })
        .expect(200);

      // The booleans must arrive as real booleans, not the strings a query string carries.
      expect(mockAssetDIDService.list).toHaveBeenCalledWith({
        communityUuid: communityUuid('Pilot1'),
        subjectType: AssetDIDSubjectType.ASSET,
        registeredOnChain: false,
        retired: false,
      });
    });

    it('delegates an empty query as an empty filter', async () => {
      mockAssetDIDService.list.mockResolvedValue([]);

      await request(app.getHttpServer()).get('/asset-dids').expect(200);

      expect(mockAssetDIDService.list).toHaveBeenCalledWith({});
    });

    it('rejects an unknown query parameter', async () => {
      await request(app.getHttpServer())
        .get('/asset-dids')
        .query({ assetName: 'LIC08SM' })
        .expect(400);

      expect(mockAssetDIDService.list).not.toHaveBeenCalled();
    });

    it('rejects an invalid subjectType', async () => {
      await request(app.getHttpServer())
        .get('/asset-dids')
        .query({ subjectType: 'meter' })
        .expect(400);
    });

    it('rejects a non-boolean retired filter', async () => {
      await request(app.getHttpServer()).get('/asset-dids').query({ retired: 'yes' }).expect(400);
    });

    it('returns whatever the service returns', async () => {
      const records = [{ subjectUuid: assetItem.subjectUuid, did: 'did:ethr:0xabc' }];
      mockAssetDIDService.list.mockResolvedValue(records);

      const result = await controller.list({});

      expect(result).toBe(records);
    });
  });

  describe('phase scoping', () => {
    it('exposes only the three phase-1 routes', () => {
      const handlers = Object.getOwnPropertyNames(AssetDIDController.prototype).filter(
        (name) => name !== 'constructor',
      );

      // register / register-batch (phase 3) and credential (phase 4) are deliberately absent.
      expect(handlers.sort()).toEqual(['findBySubjectUuid', 'list', 'sync']);
    });
  });
});
