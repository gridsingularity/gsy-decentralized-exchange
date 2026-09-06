import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication, ValidationPipe } from '@nestjs/common';
import request from 'supertest';
import { getModelToken } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { v5 as uuidv5 } from 'uuid';
import { AppModule } from '../src/app.module';
import { AssetDID, AssetDIDSubjectType, AuditLog } from '../src/database/schemas';

// ApiKeyGuard fails closed: AppModule refuses to boot without a configured API key
// (plan §2.6). These e2e specs build the whole AppModule, so give them one.
process.env.API_KEY = process.env.API_KEY || 'test_api_key';
// AssetKeyService likewise refuses to start without a valid master seed.
// Obviously-fake fixed test seed - never a real one.
process.env.ASSET_DID_MASTER_SEED =
  process.env.ASSET_DID_MASTER_SEED || '01'.repeat(32);

/** `Uuid::NAMESPACE_OID` - the namespace gsy-community-client derives ids in. */
const IDENTITY_NAMESPACE = '6ba7b812-9dad-11d1-80b4-00c04fd430c8';
const communityUuidOf = (name: string) => uuidv5(name, IDENTITY_NAMESPACE);
const areaUuidOf = (community: string, asset: string) =>
  uuidv5(`${community}:${asset}`, IDENTITY_NAMESPACE);

/**
 * A test-only community, so this suite can never be mistaken for, or collide with, a real
 * Pilot1/2/3 sync against a shared database.
 */
const TEST_COMMUNITY = 'E2ETestPilot';

const communityItem = {
  subjectUuid: communityUuidOf(TEST_COMMUNITY),
  communityName: TEST_COMMUNITY,
  communityUuid: communityUuidOf(TEST_COMMUNITY),
};

const assetItems = ['E2E01SM', 'E2E02PV', 'E2E03BAT'].map((assetName) => ({
  subjectUuid: areaUuidOf(TEST_COMMUNITY, assetName),
  assetName,
  assetType: 'SMART_METER',
  communityName: TEST_COMMUNITY,
  communityUuid: communityUuidOf(TEST_COMMUNITY),
  areaHash: 'cd'.repeat(32),
}));

const syncPayload = { communities: [communityItem], assets: assetItems };
const allSubjectUuids = [communityItem.subjectUuid, ...assetItems.map((a) => a.subjectUuid)];

describe('Asset DID sync and lookup (e2e)', () => {
  let app: INestApplication;
  let assetDidModel: Model<AssetDID>;
  let auditLogModel: Model<AuditLog>;
  let apiKey: string;

  jest.setTimeout(180000);

  const post = (payload: object) =>
    request(app.getHttpServer())
      .post('/asset-dids/sync')
      .set('x-api-key', apiKey)
      .send(payload);

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule],
    }).compile();

    app = moduleFixture.createNestApplication();
    // Same pipe configuration as `main.ts:10-16`.
    app.useGlobalPipes(
      new ValidationPipe({ whitelist: true, transform: true, forbidNonWhitelisted: true }),
    );
    await app.init();

    assetDidModel = app.get<Model<AssetDID>>(getModelToken(AssetDID.name));
    auditLogModel = app.get<Model<AuditLog>>(getModelToken(AuditLog.name));
    apiKey = process.env.API_KEY;

    await cleanup();
  }, 120000);

  afterAll(async () => {
    try {
      await cleanup();
    } catch (error) {
      console.error('Error cleaning up asset DID test data:', error);
    }
    if (app) await app.close();
  }, 120000);

  async function cleanup(): Promise<void> {
    const docs = await assetDidModel.find({ subjectUuid: { $in: allSubjectUuids } }).lean().exec();
    await assetDidModel.deleteMany({ subjectUuid: { $in: allSubjectUuids } }).exec();
    if (docs.length > 0) {
      await auditLogModel.deleteMany({ did: { $in: docs.map((d) => d.did) } }).exec();
    }
  }

  describe('POST /asset-dids/sync', () => {
    it('creates one record per subject and returns the subjectUuid -> did map', async () => {
      const response = await post(syncPayload).expect(200);

      expect(response.body.created).toBe(4);
      expect(response.body.updated).toBe(0);
      expect(response.body.retired).toBe(0);
      expect(response.body.subjects).toHaveLength(4);

      response.body.subjects.forEach((subject: any) => {
        expect(subject.did).toMatch(/^did:ethr:0x[0-9a-f]{40}$/);
        expect(subject.registeredOnChain).toBe(false);
      });

      // Acceptance criterion 3a: the community DID is distinct from every asset DID.
      expect(new Set(response.body.subjects.map((s: any) => s.did)).size).toBe(4);

      const stored = await assetDidModel
        .find({ subjectUuid: { $in: allSubjectUuids } })
        .lean()
        .exec();
      expect(stored).toHaveLength(4);
      stored.forEach((doc: any) => {
        expect(doc).not.toHaveProperty('privateKey');
        expect(doc.did).toBe(`did:ethr:${doc.address}`);
      });
    });

    it('is idempotent: a second identical sync creates nothing and returns the same DIDs', async () => {
      const first = await post(syncPayload).expect(200);
      const second = await post(syncPayload).expect(200);

      expect(second.body.created).toBe(0);
      expect(second.body.updated).toBe(4);

      const firstDids = Object.fromEntries(
        first.body.subjects.map((s: any) => [s.subjectUuid, s.did]),
      );
      const secondDids = Object.fromEntries(
        second.body.subjects.map((s: any) => [s.subjectUuid, s.did]),
      );
      expect(secondDids).toEqual(firstDids);
    });

    /**
     * THE regenerability criterion (plan §3 phase 1, acceptance criterion 3), and the single
     * most important assertion in this unit: Mongo is a cache, not the source of truth. The
     * master seed plus the ontology must reproduce the identical identities after total data
     * loss - otherwise a DID stops meaning anything the moment the database is rebuilt, and
     * every certificate issued against it is orphaned.
     *
     * Destructive by design: it drops the whole `assetdids` collection, so this suite
     * expects a disposable test database (docker-compose.test.yml).
     */
    it('reproduces byte-identical DIDs after the collection is dropped', async () => {
      const before = await post(syncPayload).expect(200);

      try {
        await assetDidModel.collection.drop();
      } catch (error: any) {
        // 26 = NamespaceNotFound: nothing to drop is a fine starting point.
        if (error?.code !== 26) throw error;
      }
      expect(await assetDidModel.countDocuments({}).exec()).toBe(0);

      const after = await post(syncPayload).expect(200);

      // Everything is a create again - the collection really was empty.
      expect(after.body.created).toBe(4);

      const beforeDids = Object.fromEntries(
        before.body.subjects.map((s: any) => [s.subjectUuid, s.did]),
      );
      const afterDids = Object.fromEntries(
        after.body.subjects.map((s: any) => [s.subjectUuid, s.did]),
      );
      expect(afterDids).toEqual(beforeDids);

      // And the persisted addresses came back identical too, not just the response.
      const stored = await assetDidModel
        .find({ subjectUuid: { $in: allSubjectUuids } })
        .lean()
        .exec();
      stored.forEach((doc: any) => {
        expect(doc.did).toBe(beforeDids[doc.subjectUuid]);
        expect(doc.address).toBe(beforeDids[doc.subjectUuid].substring('did:ethr:'.length));
      });
    });

    it('retires a subject absent from a later sync without deleting it', async () => {
      await post(syncPayload).expect(200);

      const partial = {
        communities: [communityItem],
        assets: assetItems.slice(0, 2),
      };
      const response = await post(partial).expect(200);

      expect(response.body.retired).toBe(1);

      const retiredUuid = assetItems[2].subjectUuid;
      const retiredDoc = await assetDidModel.findOne({ subjectUuid: retiredUuid }).lean().exec();

      // Still there, still resolvable - certificates issued against it stay verifiable.
      expect(retiredDoc).not.toBeNull();
      expect((retiredDoc as any).retired).toBe(true);
      expect((retiredDoc as any).did).toMatch(/^did:ethr:0x[0-9a-f]{40}$/);

      // And it comes back with the same DID.
      const revived = await post(syncPayload).expect(200);
      expect(revived.body.created).toBe(0);
      const revivedDoc = await assetDidModel.findOne({ subjectUuid: retiredUuid }).lean().exec();
      expect((revivedDoc as any).retired).toBe(false);
      expect((revivedDoc as any).did).toBe((retiredDoc as any).did);
    });

    it('rejects a payload with an undeclared field', async () => {
      await post({ ...syncPayload, surprise: true }).expect(400);
    });
  });

  describe('GET /asset-dids', () => {
    beforeEach(async () => {
      await post(syncPayload).expect(200);
    });

    it('resolves a single record by subjectUuid', async () => {
      const subjectUuid = assetItems[0].subjectUuid;

      const response = await request(app.getHttpServer())
        .get(`/asset-dids/${subjectUuid}`)
        .set('x-api-key', apiKey)
        .expect(200);

      expect(response.body.subjectUuid).toBe(subjectUuid);
      expect(response.body.subjectType).toBe(AssetDIDSubjectType.ASSET);
      expect(response.body.assetName).toBe(assetItems[0].assetName);
      expect(response.body.did).toMatch(/^did:ethr:0x[0-9a-f]{40}$/);
      // The read model must never carry key material.
      expect(response.body).not.toHaveProperty('privateKey');
    });

    it('404s for a subject no sync has mentioned', async () => {
      await request(app.getHttpServer())
        .get(`/asset-dids/${areaUuidOf(TEST_COMMUNITY, 'NEVER_SYNCED')}`)
        .set('x-api-key', apiKey)
        .expect(404);
    });

    it('lists by communityUuid and by subjectType', async () => {
      const all = await request(app.getHttpServer())
        .get('/asset-dids')
        .query({ communityUuid: communityUuidOf(TEST_COMMUNITY) })
        .set('x-api-key', apiKey)
        .expect(200);

      expect(all.body).toHaveLength(4);

      const communitiesOnly = await request(app.getHttpServer())
        .get('/asset-dids')
        .query({
          communityUuid: communityUuidOf(TEST_COMMUNITY),
          subjectType: AssetDIDSubjectType.COMMUNITY,
        })
        .set('x-api-key', apiKey)
        .expect(200);

      expect(communitiesOnly.body).toHaveLength(1);
      expect(communitiesOnly.body[0].subjectUuid).toBe(communityItem.subjectUuid);
    });
  });

  describe('GET /did/:did on a synced asset', () => {
    it('resolves to the zero-transaction default DID document', async () => {
      // Phase 1 writes nothing on-chain: a did:ethr is already a valid, resolvable identity
      // for any address with no registry transaction at all (plan §2.3).
      const response = await post(syncPayload).expect(200);
      const did = response.body.subjects[0].did;

      const resolved = await request(app.getHttpServer()).get(`/did/${did}`).expect(200);

      expect(resolved.body.id).toBe(did);
      expect(resolved.body.verificationMethod[0].id).toBe(`${did}#controller`);
      expect(resolved.body.verificationMethod[0]).toHaveProperty('blockchainAccountId');
    });
  });

  describe('auth', () => {
    it('401s on POST /asset-dids/sync without an x-api-key header', async () => {
      const response = await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .send(syncPayload)
        .expect(401);

      expect(response.body.message).toBe('invalid or missing x-api-key');
    });

    it('401s with a wrong x-api-key', async () => {
      await request(app.getHttpServer())
        .post('/asset-dids/sync')
        .set('x-api-key', 'definitely-not-the-key')
        .send(syncPayload)
        .expect(401);
    });

    it('401s on the GET routes without a key', async () => {
      await request(app.getHttpServer()).get('/asset-dids').expect(401);
      await request(app.getHttpServer())
        .get(`/asset-dids/${assetItems[0].subjectUuid}`)
        .expect(401);
    });
  });
});
