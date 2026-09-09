import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getModelToken } from '@nestjs/mongoose';
import { BadRequestException, ConflictException, NotFoundException } from '@nestjs/common';
import { v5 as uuidv5 } from 'uuid';
import { AssetDIDService } from '../src/assets/asset-did.service';
import { AssetKeyService } from '../src/assets/asset-key.service';
import { AuditService } from '../src/audit/audit.service';
import { CredentialsService } from '../src/credentials/credentials.service';
import { AssetDID, AssetDIDSubjectType, AuditAction } from '../src/database/schemas';
import { AssetSyncRequest } from '../src/assets/dto/asset-sync-request.dto';

/**
 * `Uuid::NAMESPACE_OID`, the namespace `gsy-community-client` derives every canonical id
 * in (`offchain_storage_connector/adapter.rs:38`). Used here so the fixtures have the same
 * shape as the real payload rather than being arbitrary strings.
 */
const IDENTITY_NAMESPACE = '6ba7b812-9dad-11d1-80b4-00c04fd430c8';

const communityUuid = (community: string) => uuidv5(community, IDENTITY_NAMESPACE);
const areaUuid = (community: string, asset: string) =>
  uuidv5(`${community}:${asset}`, IDENTITY_NAMESPACE);

/** Obviously-fake fixed test seed. Never a real one. */
const TEST_SEED = '01'.repeat(32);

// ---------------------------------------------------------------------------------------
// A tiny in-memory stand-in for the Mongo model.
//
// A plain `jest.fn()` returning canned values cannot express what this unit is actually
// about - idempotency, retirement and un-retirement are all statements about what the
// SECOND sync sees. So the fake keeps a real array and applies the bulkWrite operations to
// it, which lets a test run two syncs back to back and assert on the resulting documents.
// It supports exactly the query operators the service uses: `$or`, `$in`, `$nin`.
// ---------------------------------------------------------------------------------------

type StoredDoc = Record<string, any>;

function matchesQuery(doc: StoredDoc, query: Record<string, any>): boolean {
  return Object.entries(query).every(([key, condition]) => {
    if (key === '$or') {
      return (condition as Record<string, any>[]).some((sub) => matchesQuery(doc, sub));
    }
    if (condition !== null && typeof condition === 'object' && !Array.isArray(condition)) {
      if ('$in' in condition) return (condition.$in as any[]).includes(doc[key]);
      if ('$nin' in condition) return !(condition.$nin as any[]).includes(doc[key]);
    }
    return doc[key] === condition;
  });
}

function createModelMock(store: StoredDoc[]) {
  const query = (results: () => StoredDoc[]) => {
    const chain: any = {
      sort: jest.fn(() => chain),
      lean: jest.fn(() => chain),
      exec: jest.fn(async () => results()),
    };
    return chain;
  };

  const model: any = {
    find: jest.fn((filter: Record<string, any> = {}) =>
      query(() => store.filter((doc) => matchesQuery(doc, filter)).map((doc) => ({ ...doc }))),
    ),
    findOne: jest.fn((filter: Record<string, any> = {}) => {
      const chain: any = {
        lean: jest.fn(() => chain),
        exec: jest.fn(async () => {
          const found = store.find((doc) => matchesQuery(doc, filter));
          return found ? { ...found } : null;
        }),
      };
      return chain;
    }),
    updateOne: jest.fn((filter: Record<string, any>, update: Record<string, any>) => ({
      exec: jest.fn(async () => {
        const target = store.find((doc) => matchesQuery(doc, filter));
        if (target) Object.assign(target, update.$set);
        return { acknowledged: true, modifiedCount: target ? 1 : 0 };
      }),
    })),
    bulkWrite: jest.fn(async (operations: any[]) => {
      for (const operation of operations) {
        if (operation.insertOne) {
          store.push({ ...operation.insertOne.document });
        } else if (operation.updateOne) {
          const target = store.find((doc) => matchesQuery(doc, operation.updateOne.filter));
          if (target) Object.assign(target, operation.updateOne.update.$set);
        } else {
          throw new Error(`unexpected bulkWrite operation: ${Object.keys(operation).join(',')}`);
        }
      }
      return { ok: 1 };
    }),
  };

  return model;
}

/** Recursively hunt for anything key-shaped. Returns a human-readable path on a hit. */
function findKeyMaterial(value: unknown, path = '$'): string | null {
  if (typeof value === 'string') {
    // A secp256k1 private key as ethers renders it: 0x + 64 hex chars.
    return /^0x[0-9a-fA-F]{64}$/.test(value) ? `${path} = ${value}` : null;
  }
  if (Array.isArray(value)) {
    for (let i = 0; i < value.length; i += 1) {
      const hit = findKeyMaterial(value[i], `${path}[${i}]`);
      if (hit) return hit;
    }
    return null;
  }
  if (value !== null && typeof value === 'object') {
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      if (/priv(ate)?key|secret|seed|mnemonic/i.test(key)) {
        return `${path}.${key} (forbidden field name)`;
      }
      const hit = findKeyMaterial(child, `${path}.${key}`);
      if (hit) return hit;
    }
  }
  return null;
}

describe('AssetDIDService', () => {
  let service: AssetDIDService;
  let keyService: AssetKeyService;
  let model: any;
  let store: StoredDoc[];
  let mockAuditService: { log: jest.Mock };
  let mockCredentialsService: { issueAssetCredential: jest.Mock };

  const PILOT1 = 'Pilot1';
  const PILOT2 = 'Pilot2';

  const community = (name: string) => ({
    subjectUuid: communityUuid(name),
    communityName: name,
    communityUuid: communityUuid(name),
  });

  const asset = (communityName: string, assetName: string) => ({
    subjectUuid: areaUuid(communityName, assetName),
    assetName,
    assetType: 'SMART_METER',
    communityName,
    communityUuid: communityUuid(communityName),
    areaHash: 'ab'.repeat(32),
  });

  beforeEach(async () => {
    jest.clearAllMocks();

    store = [];
    model = createModelMock(store);
    mockAuditService = { log: jest.fn().mockResolvedValue(true) };
    mockCredentialsService = {
      issueAssetCredential: jest.fn().mockResolvedValue({
        id: 'urn:uuid:issued-asset-credential',
        credential: { proof: { jws: '0xsig' } },
      }),
    };

    const mockConfigService = {
      get: jest.fn((key: string) => {
        if (key === 'assetDid.masterSeed') return TEST_SEED;
        if (key === 'assetDid.derivationVersion') return 1;
        return null;
      }),
    };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AssetDIDService,
        AssetKeyService,
        { provide: getModelToken(AssetDID.name), useValue: model },
        { provide: ConfigService, useValue: mockConfigService },
        { provide: AuditService, useValue: mockAuditService },
        { provide: CredentialsService, useValue: mockCredentialsService },
      ],
    }).compile();

    service = module.get<AssetDIDService>(AssetDIDService);
    // The real key service, not a mock: the DIDs under test have to be the real derived
    // ones for "byte-identical across syncs" to mean anything.
    keyService = module.get<AssetKeyService>(AssetKeyService);
    keyService.onModuleInit();
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('syncSubjects - creation', () => {
    it('creates one record per subject on the first sync', async () => {
      const payload: AssetSyncRequest = {
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest;

      const result = await service.syncSubjects(payload);

      expect(result.created).toBe(3);
      expect(result.updated).toBe(0);
      expect(result.retired).toBe(0);
      expect(result.subjects).toHaveLength(3);
      expect(store).toHaveLength(3);
    });

    it('stamps every created record with a derived did, address, path and version', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const stored = store[0];
      const expected = keyService.derive(areaUuid(PILOT1, 'LIC08SM'));

      expect(stored.did).toBe(expected.did);
      expect(stored.address).toBe(expected.address);
      expect(stored.derivationPath).toBe(expected.derivationPath);
      expect(stored.derivationVersion).toBe(1);
      expect(stored.did).toBe(`did:ethr:${stored.address}`);
      expect(stored.registeredOnChain).toBe(false);
      expect(stored.retired).toBe(false);
      expect(stored.lastSeenAt).toBeInstanceOf(Date);
    });

    it('creates both subject types from a mixed payload with the right subjectType', async () => {
      const payload = {
        communities: [community(PILOT1), community(PILOT2)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT2, 'GD01SM')],
      } as AssetSyncRequest;

      const result = await service.syncSubjects(payload);

      expect(result.created).toBe(4);

      const communities = store.filter((d) => d.subjectType === AssetDIDSubjectType.COMMUNITY);
      const assets = store.filter((d) => d.subjectType === AssetDIDSubjectType.ASSET);

      expect(communities).toHaveLength(2);
      expect(assets).toHaveLength(2);

      // Asset-only fields are absent on a community record, not null (plan §2.5).
      expect(communities[0]).not.toHaveProperty('assetName');
      expect(communities[0]).not.toHaveProperty('assetType');
      expect(communities[0]).not.toHaveProperty('areaHash');
      expect(communities[0].communityUuid).toBe(communities[0].subjectUuid);

      expect(assets[0].assetName).toBe('LIC08SM');
      expect(assets[0].assetType).toBe('SMART_METER');
      expect(assets[0].areaHash).toBe('ab'.repeat(32));

      // Acceptance criterion 3a: community DIDs are distinct from asset DIDs.
      expect(new Set(store.map((d) => d.did)).size).toBe(4);

      const byType = new Map(result.subjects.map((s) => [s.subjectUuid, s.subjectType]));
      expect(byType.get(communityUuid(PILOT1))).toBe(AssetDIDSubjectType.COMMUNITY);
      expect(byType.get(areaUuid(PILOT1, 'LIC08SM'))).toBe(AssetDIDSubjectType.ASSET);
    });

    it('works with an assets-only payload and with a communities-only payload', async () => {
      const assetsOnly = await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);
      expect(assetsOnly.created).toBe(1);

      const communitiesOnly = await service.syncSubjects({
        communities: [community(PILOT1)],
      } as AssetSyncRequest);
      expect(communitiesOnly.created).toBe(1);
      expect(store).toHaveLength(2);
    });
  });

  describe('syncSubjects - idempotency', () => {
    it('creates 0 and updates N on an identical second sync, with identical DIDs', async () => {
      const payload = {
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest;

      const first = await service.syncSubjects(payload);
      const second = await service.syncSubjects(payload);

      expect(first.created).toBe(3);
      expect(second.created).toBe(0);
      expect(second.updated).toBe(3);
      expect(second.retired).toBe(0);
      expect(store).toHaveLength(3);

      // Byte-identical, per subject - not merely the same multiset of DIDs.
      const firstDids = new Map(first.subjects.map((s) => [s.subjectUuid, s.did]));
      const secondDids = new Map(second.subjects.map((s) => [s.subjectUuid, s.did]));
      expect(secondDids).toEqual(firstDids);
    });

    it('does not throw on a re-sync (contrast did.service.ts:92-95)', async () => {
      const payload = { assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest;

      await service.syncSubjects(payload);
      await expect(service.syncSubjects(payload)).resolves.toBeDefined();
    });

    it('bumps lastSeenAt on every subject present in the payload', async () => {
      const payload = { assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest;

      await service.syncSubjects(payload);
      const firstSeen: Date = store[0].lastSeenAt;

      await new Promise((resolve) => setTimeout(resolve, 5));
      await service.syncSubjects(payload);

      expect(store[0].lastSeenAt.getTime()).toBeGreaterThan(firstSeen.getTime());
    });
  });

  describe('syncSubjects - retirement', () => {
    it('retires an absent subject instead of deleting it', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);

      const result = await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      expect(result.retired).toBe(1);
      // Never deleted: a retired DID must stay resolvable for certificates already issued
      // against it (plan §2.2).
      expect(store).toHaveLength(2);

      const gone = store.find((d) => d.subjectUuid === areaUuid(PILOT1, 'LIC09SM'));
      expect(gone.retired).toBe(true);
      expect(gone.did).toBeDefined();

      const kept = store.find((d) => d.subjectUuid === areaUuid(PILOT1, 'LIC08SM'));
      expect(kept.retired).toBe(false);
    });

    it('never issues a delete operation', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const allOperations = model.bulkWrite.mock.calls.flatMap((call: any[]) => call[0]);
      const operationNames = new Set(allOperations.flatMap((op: any) => Object.keys(op)));

      expect(operationNames).toEqual(new Set(['insertOne', 'updateOne']));
    });

    it('does not re-retire an already-retired subject on the next sync', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const third = await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      expect(third.retired).toBe(0);
    });

    it('un-retires a re-appearing subject and keeps the same DID', async () => {
      const full = {
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest;

      const first = await service.syncSubjects(full);
      const originalDid = first.subjects.find(
        (s) => s.subjectUuid === areaUuid(PILOT1, 'LIC09SM'),
      ).did;

      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      expect(store.find((d) => d.subjectUuid === areaUuid(PILOT1, 'LIC09SM')).retired).toBe(true);

      const third = await service.syncSubjects(full);

      // Re-derived, not re-minted.
      expect(third.created).toBe(0);
      expect(third.updated).toBe(2);
      const revived = store.find((d) => d.subjectUuid === areaUuid(PILOT1, 'LIC09SM'));
      expect(revived.retired).toBe(false);
      expect(revived.did).toBe(originalDid);
      expect(
        third.subjects.find((s) => s.subjectUuid === areaUuid(PILOT1, 'LIC09SM')).did,
      ).toBe(originalDid);
    });

    it('scopes retirement to the communities the payload covers', async () => {
      await service.syncSubjects({
        communities: [community(PILOT1), community(PILOT2)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT2, 'GD01SM'), asset(PILOT2, 'GD02SM')],
      } as AssetSyncRequest);

      // A sync that knows only about Pilot1 must not touch Pilot2.
      const result = await service.syncSubjects({
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      expect(result.retired).toBe(0);
      expect(store.filter((d) => d.retired === true)).toHaveLength(0);
      expect(store.find((d) => d.subjectUuid === areaUuid(PILOT2, 'GD01SM')).retired).toBe(false);
      expect(store.find((d) => d.subjectUuid === communityUuid(PILOT2)).retired).toBe(false);
    });

    it('scopes retirement by subject type, so an assets-only sync spares community records', async () => {
      await service.syncSubjects({
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      const result = await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      expect(result.retired).toBe(0);
      expect(store.find((d) => d.subjectUuid === communityUuid(PILOT1)).retired).toBe(false);
    });
  });

  describe('syncSubjects - collision detection', () => {
    it('throws rather than overwriting when a derived address is held by another subject', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      const victim = { ...store[0] };

      // Force the ~10^-13 event (plan §2.2): a different subject deriving the stored address.
      jest.spyOn(keyService, 'derive').mockReturnValue({
        did: victim.did,
        address: victim.address,
        privateKey: `0x${'11'.repeat(32)}`,
        derivationPath: victim.derivationPath,
        derivationVersion: 1,
      });

      model.bulkWrite.mockClear();

      await expect(
        service.syncSubjects({ assets: [asset(PILOT1, 'IMPOSTER')] } as AssetSyncRequest),
      ).rejects.toThrow(ConflictException);

      // Loud failure means NOTHING was written - not a partial sync.
      expect(model.bulkWrite).not.toHaveBeenCalled();
      expect(store).toHaveLength(1);
      expect(store[0].subjectUuid).toBe(victim.subjectUuid);
    });

    it('throws when two subjects inside one payload derive the same address', async () => {
      const victim = keyService.derive(areaUuid(PILOT1, 'LIC08SM'));
      jest.spyOn(keyService, 'derive').mockReturnValue({ ...victim });

      await expect(
        service.syncSubjects({
          assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
        } as AssetSyncRequest),
      ).rejects.toThrow(ConflictException);

      expect(model.bulkWrite).not.toHaveBeenCalled();
      expect(store).toHaveLength(0);
    });

    it('does not treat a subject re-deriving its own stored address as a collision', async () => {
      const payload = { assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest;

      await service.syncSubjects(payload);
      await expect(service.syncSubjects(payload)).resolves.toMatchObject({ created: 0 });
    });
  });

  describe('syncSubjects - rejected payloads', () => {
    it('rejects a payload with neither communities nor assets', async () => {
      await expect(service.syncSubjects({} as AssetSyncRequest)).rejects.toThrow(
        BadRequestException,
      );
      await expect(
        service.syncSubjects({ communities: [], assets: [] } as AssetSyncRequest),
      ).rejects.toThrow(BadRequestException);
      expect(model.bulkWrite).not.toHaveBeenCalled();
    });

    it('rejects a payload repeating the same subjectUuid', async () => {
      await expect(
        service.syncSubjects({
          assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC08SM')],
        } as AssetSyncRequest),
      ).rejects.toThrow(BadRequestException);
      expect(model.bulkWrite).not.toHaveBeenCalled();
    });
  });

  describe('syncSubjects - audit trail', () => {
    it('logs ASSET_DID_CREATED once per newly created subject', async () => {
      await service.syncSubjects({
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest);

      expect(mockAuditService.log).toHaveBeenCalledTimes(2);
      const actions = mockAuditService.log.mock.calls.map((call) => call[0]);
      expect(actions).toEqual([AuditAction.ASSET_DID_CREATED, AuditAction.ASSET_DID_CREATED]);

      const dids = mockAuditService.log.mock.calls.map((call) => call[1]);
      expect(new Set(dids).size).toBe(2);
      dids.forEach((did) => expect(did).toMatch(/^did:ethr:0x[0-9a-f]{40}$/));
    });

    it('logs nothing at all for a sync that only re-confirms existing subjects', async () => {
      const payload = {
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM')],
      } as AssetSyncRequest;

      await service.syncSubjects(payload);
      mockAuditService.log.mockClear();

      await service.syncSubjects(payload);

      // 590 subjects re-confirmed hourly must not produce 590 audit rows an hour.
      expect(mockAuditService.log).not.toHaveBeenCalled();
    });

    it('logs ASSET_DID_RETIRED once, when a subject is first retired', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);
      mockAuditService.log.mockClear();

      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      expect(mockAuditService.log).toHaveBeenCalledTimes(1);
      expect(mockAuditService.log.mock.calls[0][0]).toBe(AuditAction.ASSET_DID_RETIRED);

      mockAuditService.log.mockClear();
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      expect(mockAuditService.log).not.toHaveBeenCalled();
    });
  });

  describe('key material containment', () => {
    /**
     * The regression test for a future refactor that casually persists the whole return
     * value of `AssetKeyService.derive()`.
     *
     * Asserted against what is actually handed to the model and the audit service, not
     * against the schema definition - a schema without a `privateKey` field would not stop
     * `strict: false`-style leakage, and the point is to catch the *call*, not the type.
     */
    it('never passes a private key to the model, the audit log, or the response', async () => {
      const payload = {
        communities: [community(PILOT1)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest;

      const first = await service.syncSubjects(payload);
      // Second sync exercises the updateOne branch, third exercises retirement.
      await service.syncSubjects(payload);
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const bulkWriteArgs = model.bulkWrite.mock.calls;
      expect(bulkWriteArgs.length).toBeGreaterThan(0);
      expect(findKeyMaterial(bulkWriteArgs, 'bulkWrite')).toBeNull();

      const auditArgs = mockAuditService.log.mock.calls;
      expect(auditArgs.length).toBeGreaterThan(0);
      expect(findKeyMaterial(auditArgs, 'auditService.log')).toBeNull();

      expect(findKeyMaterial(first, 'syncResponse')).toBeNull();
      expect(findKeyMaterial(store, 'persistedDocuments')).toBeNull();

      const read = await service.list();
      expect(findKeyMaterial(read, 'listResponse')).toBeNull();

      // Sanity check that the detector is not vacuous: it must flag a real derived key.
      const leaked = keyService.derive(areaUuid(PILOT1, 'LIC08SM'));
      expect(findKeyMaterial([leaked], 'control')).not.toBeNull();
    });
  });

  describe('findBySubjectUuid', () => {
    it('returns the record for a known subject', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const found = await service.findBySubjectUuid(areaUuid(PILOT1, 'LIC08SM'));

      expect(found.subjectUuid).toBe(areaUuid(PILOT1, 'LIC08SM'));
      expect(found.subjectType).toBe(AssetDIDSubjectType.ASSET);
      expect(found.did).toBe(store[0].did);
      expect(found).not.toHaveProperty('privateKey');
    });

    it('still returns a retired record, because its DID must stay resolvable', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      const found = await service.findBySubjectUuid(areaUuid(PILOT1, 'LIC09SM'));

      expect(found.retired).toBe(true);
      expect(found.did).toBeDefined();
    });

    it('throws NotFound for a subject no sync has ever mentioned', async () => {
      await expect(service.findBySubjectUuid(areaUuid(PILOT1, 'NOPE'))).rejects.toThrow(
        NotFoundException,
      );
    });
  });

  describe('issueCredential (phase 4)', () => {
    it('issues for a known asset with the record\'s DID and its ontology claims', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      const stored = store[0];

      const result = await service.issueCredential(areaUuid(PILOT1, 'LIC08SM'));

      expect(mockCredentialsService.issueAssetCredential).toHaveBeenCalledWith(
        stored.did,
        {
          subjectUuid: areaUuid(PILOT1, 'LIC08SM'),
          communityName: PILOT1,
          communityUuid: communityUuid(PILOT1),
          assetName: 'LIC08SM',
          assetType: 'SMART_METER',
        },
        undefined,
      );
      expect(result.id).toBe('urn:uuid:issued-asset-credential');
    });

    it('sets hasAssetCredential on the record', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      expect(store[0].hasAssetCredential).toBe(false);

      await service.issueCredential(areaUuid(PILOT1, 'LIC08SM'));

      expect(store[0].hasAssetCredential).toBe(true);
      // NOT the human flag. `User.hasVerifiedCredential` means a principal proved
      // possession of two keys; this means the issuer attested about a machine subject
      // that signed nothing (plan §2.7).
      expect(store[0]).not.toHaveProperty('hasVerifiedCredential');
    });

    it('leaves the flag false when issuance fails, never the other way round', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      mockCredentialsService.issueAssetCredential.mockRejectedValueOnce(new Error('signing failed'));

      await expect(service.issueCredential(areaUuid(PILOT1, 'LIC08SM'))).rejects.toThrow(
        'signing failed',
      );

      // A true flag with no credential behind it is the one inconsistency an operator
      // cannot spot from this collection; a false flag with a credential is recoverable.
      expect(store[0].hasAssetCredential).toBe(false);
    });

    it('omits the asset-only claims for a community subject', async () => {
      await service.syncSubjects({ communities: [community(PILOT1)] } as AssetSyncRequest);

      await service.issueCredential(communityUuid(PILOT1));

      const claims = mockCredentialsService.issueAssetCredential.mock.calls[0][1];
      expect(claims.assetName).toBeUndefined();
      expect(claims.assetType).toBeUndefined();
      expect(claims.communityUuid).toBe(communityUuid(PILOT1));
    });

    it('throws NotFound for a subject no sync has ever mentioned', async () => {
      await expect(service.issueCredential(areaUuid(PILOT1, 'NOPE'))).rejects.toThrow(
        NotFoundException,
      );
      expect(mockCredentialsService.issueAssetCredential).not.toHaveBeenCalled();
    });

    it('issues for a retired subject: retirement does not invalidate its identity', async () => {
      await service.syncSubjects({
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM')],
      } as AssetSyncRequest);
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);

      await expect(service.issueCredential(areaUuid(PILOT1, 'LIC09SM'))).resolves.toBeDefined();
    });

    it('passes the request through so the audit trail records the caller', async () => {
      await service.syncSubjects({ assets: [asset(PILOT1, 'LIC08SM')] } as AssetSyncRequest);
      const req = { ip: '127.0.0.1' } as any;

      await service.issueCredential(areaUuid(PILOT1, 'LIC08SM'), req);

      expect(mockCredentialsService.issueAssetCredential).toHaveBeenCalledWith(
        expect.any(String),
        expect.any(Object),
        req,
      );
    });
  });

  describe('list', () => {
    beforeEach(async () => {
      await service.syncSubjects({
        communities: [community(PILOT1), community(PILOT2)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT1, 'LIC09SM'), asset(PILOT2, 'GD01SM')],
      } as AssetSyncRequest);
    });

    it('lists everything with no filter', async () => {
      expect(await service.list()).toHaveLength(5);
      expect(await service.list({})).toHaveLength(5);
    });

    it('filters by communityUuid', async () => {
      const results = await service.list({ communityUuid: communityUuid(PILOT1) });
      expect(results).toHaveLength(3);
      results.forEach((r) => expect(r.communityUuid).toBe(communityUuid(PILOT1)));
    });

    it('filters by subjectType', async () => {
      expect(await service.list({ subjectType: AssetDIDSubjectType.COMMUNITY })).toHaveLength(2);
      expect(await service.list({ subjectType: AssetDIDSubjectType.ASSET })).toHaveLength(3);
    });

    it('filters by registeredOnChain', async () => {
      // Phase 1 writes nothing on-chain, so every record is false (plan §2.3).
      expect(await service.list({ registeredOnChain: false })).toHaveLength(5);
      expect(await service.list({ registeredOnChain: true })).toHaveLength(0);
    });

    it('filters by retired', async () => {
      await service.syncSubjects({
        communities: [community(PILOT1), community(PILOT2)],
        assets: [asset(PILOT1, 'LIC08SM'), asset(PILOT2, 'GD01SM')],
      } as AssetSyncRequest);

      expect(await service.list({ retired: true })).toHaveLength(1);
      expect(await service.list({ retired: false })).toHaveLength(4);
    });

    it('combines filters', async () => {
      const results = await service.list({
        communityUuid: communityUuid(PILOT2),
        subjectType: AssetDIDSubjectType.ASSET,
      });

      expect(results).toHaveLength(1);
      expect(results[0].assetName).toBe('GD01SM');
    });
  });

  describe('write volume', () => {
    it('uses one bulkWrite and two finds for the whole payload, not a round trip per subject', async () => {
      const assets = Array.from({ length: 120 }, (_, i) =>
        asset(PILOT1, `LIC${String(i).padStart(3, '0')}SM`),
      );

      await service.syncSubjects({
        communities: [community(PILOT1)],
        assets,
      } as AssetSyncRequest);

      expect(model.bulkWrite).toHaveBeenCalledTimes(1);
      expect(model.bulkWrite.mock.calls[0][0]).toHaveLength(121);
      // One lookup of existing records, one for retirement candidates.
      expect(model.find).toHaveBeenCalledTimes(2);
      expect(store).toHaveLength(121);
      // 121 distinct derived identities - the mask in derivationPathFor is doing its job.
      expect(new Set(store.map((d) => d.did)).size).toBe(121);
    });
  });
});
