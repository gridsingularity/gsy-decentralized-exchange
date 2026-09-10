import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { createHash } from 'crypto';
import { AssetKeyService } from '../src/assets/asset-key.service';

// Obviously-fake fixed test seeds. NEVER put a real seed in a test.
const TEST_SEED = '01'.repeat(32); // 32 bytes of 0x01
const OTHER_SEED = '02'.repeat(32); // 32 bytes of 0x02

/**
 *
 * This address was computed independently of AssetKeyService, in a throwaway script, by
 * three separate routes that were required to agree before it was written down here:
 *   1. `sha256("gsy-asset-did:v1:" + canonicalId)` with node's `crypto`, the two index
 *      words read with `readUInt32BE` and masked with `& 0x7fffffff` by hand, and the
 *      path string assembled by hand;
 *   2. `HDNodeWallet.fromSeed(seed).derivePath(path)` called directly on that path;
 *   3. a from-scratch BIP-32 implementation (HMAC-SHA512 + BigInt secp256k1 scalar
 *      multiplication + keccak256 of the uncompressed public key), which does not use
 *      ethers' HD wallet at all.
 *
 * DO NOT REGENERATE THIS BY CALLING `AssetKeyService.derive()`. Pasting the
 * implementation's own output back in would turn this test into a tautology that
 * happily locks in a wrong derivation. If this test fails, the derivation scheme
 * changed - and once any DID has been minted, that is a migration, not a fix.
 */
const GOLDEN = {
  seed: TEST_SEED,
  canonicalId: '5e3e4a8b-4a0f-5c3f-9c8e-0f7a1b2c3d4e',
  address: '0xe194ab62fe7f8e28f2266b317bdcfc053999fb21',
  path: "m/44'/60'/0'/2004657372/1277089372",
};

function configWith(masterSeed: any, derivationVersion: any = 1): ConfigService {
  return {
    get: jest.fn((key: string) => {
      if (key === 'assetDid.masterSeed') return masterSeed;
      if (key === 'assetDid.derivationVersion') return derivationVersion;
      return undefined;
    }),
  } as unknown as ConfigService;
}

/** Build a service through the DI container and run its lifecycle hook. */
async function makeService(
  masterSeed: any,
  derivationVersion: any = 1,
): Promise<AssetKeyService> {
  const module: TestingModule = await Test.createTestingModule({
    providers: [
      AssetKeyService,
      { provide: ConfigService, useValue: configWith(masterSeed, derivationVersion) },
    ],
  }).compile();

  const service = module.get<AssetKeyService>(AssetKeyService);
  service.onModuleInit();
  return service;
}

/** Independent (non-service) computation of the two BIP-32 child indices. */
function indicesFor(canonicalId: string): { i0: number; i1: number; raw: Buffer } {
  const h = createHash('sha256')
    .update('gsy-asset-did:v1:' + canonicalId, 'utf8')
    .digest();
  return {
    i0: h.readUInt32BE(0) & 0x7fffffff,
    i1: h.readUInt32BE(4) & 0x7fffffff,
    raw: h,
  };
}

describe('AssetKeyService', () => {
  let service: AssetKeyService;

  beforeAll(async () => {
    service = await makeService(TEST_SEED);
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('golden vector', () => {
    // See the GOLDEN comment above: computed independently, never regenerated from
    // the implementation.
    it('should derive the independently computed address for the golden pair', () => {
      const derived = service.derive(GOLDEN.canonicalId);

      expect(derived.address).toBe(GOLDEN.address);
      expect(derived.did).toBe(`did:ethr:${GOLDEN.address}`);
      expect(derived.derivationPath).toBe(GOLDEN.path);
      expect(derived.derivationVersion).toBe(1);
    });
  });

  describe('determinism', () => {
    it('should derive the same address from two independently constructed instances', async () => {
      const a = await makeService(TEST_SEED);
      const b = await makeService(TEST_SEED);
      const id = 'a1b2c3d4-0000-5000-8000-000000000001';

      expect(a).not.toBe(b);
      expect(a.derive(id).address).toBe(b.derive(id).address);
      expect(a.derive(id).did).toBe(b.derive(id).did);
      expect(a.derive(id).privateKey).toBe(b.derive(id).privateKey);
      expect(a.derive(id).derivationPath).toBe(b.derive(id).derivationPath);
    });

    it('should be stable across repeated calls on one instance', () => {
      const id = 'a1b2c3d4-0000-5000-8000-000000000002';
      expect(service.derive(id).address).toBe(service.derive(id).address);
    });
  });

  describe('seed sensitivity', () => {
    it('should derive a different address for the same id under a different seed', async () => {
      const other = await makeService(OTHER_SEED);
      const id = 'a1b2c3d4-0000-5000-8000-000000000003';

      expect(other.derive(id).address).not.toBe(service.derive(id).address);
    });
  });

  describe('distinctness', () => {
    it('should derive 1000 distinct addresses for 1000 distinct ids', () => {
      // Guards the HD-index mask: a mask bug that collapsed indices (e.g. masking to
      // too few bits) would show up here as collisions.
      const addresses = new Set<string>();
      for (let i = 0; i < 1000; i++) {
        addresses.add(service.derive(`synthetic-subject-${i}`).address);
      }
      expect(addresses.size).toBe(1000);
    }, 60000);

    it('should keep community-style ids distinct from a batch of asset-style ids', () => {
      // Mirrors plan §2.5: both subject types are v5 UUIDs in the same namespace, but
      // the preimages ("Pilot1" vs "Pilot1:LIC08SM") are never equal.
      const communityIds = ['Pilot1', 'Pilot2', 'Pilot3'].map((c) => `community:${c}`);
      const assetIds: string[] = [];
      for (const community of ['Pilot1', 'Pilot2', 'Pilot3']) {
        for (let i = 0; i < 100; i++) {
          assetIds.push(`${community}:LIC${String(i).padStart(3, '0')}SM`);
        }
      }

      const assetAddresses = new Set(assetIds.map((id) => service.derive(id).address));
      const communityAddresses = communityIds.map((id) => service.derive(id).address);

      expect(assetAddresses.size).toBe(assetIds.length);
      expect(new Set(communityAddresses).size).toBe(communityIds.length);
      for (const address of communityAddresses) {
        expect(assetAddresses.has(address)).toBe(false);
      }
    }, 60000);
  });

  describe('derivation path shape', () => {
    const pathPattern = /^m\/44'\/60'\/0'\/(\d+)\/(\d+)$/;

    it('should match m/44\'/60\'/0\'/i0/i1 with both indices below 2**31', () => {
      for (let i = 0; i < 50; i++) {
        const { derivationPath } = service.derive(`path-shape-${i}`);
        const match = derivationPath.match(pathPattern);

        expect(match).not.toBeNull();
        expect(Number(match[1])).toBeLessThan(2 ** 31);
        expect(Number(match[2])).toBeLessThan(2 ** 31);
      }
    });

    it('should mask the high bit when BOTH index words have it set', () => {
      // Find, by search rather than by assumption, an id whose sha256 has the high bit
      // set in BOTH of the first two 32-bit words. Those are exactly the ids where an
      // unmasked implementation would ask for a HARDENED child index (>= 2**31), which
      // BIP-32 reserves - so this case is what makes `& 0x7fffffff` load-bearing.
      let hardId: string | null = null;
      for (let i = 0; i < 10000 && hardId === null; i++) {
        const candidate = `mask-probe-${i}`;
        const { raw } = indicesFor(candidate);
        if (raw.readUInt32BE(0) >= 0x80000000 && raw.readUInt32BE(4) >= 0x80000000) {
          hardId = candidate;
        }
      }

      expect(hardId).not.toBeNull();

      const { raw, i0, i1 } = indicesFor(hardId);
      expect(raw.readUInt32BE(0)).toBeGreaterThanOrEqual(2 ** 31);
      expect(raw.readUInt32BE(4)).toBeGreaterThanOrEqual(2 ** 31);

      const { derivationPath, address } = service.derive(hardId);

      expect(derivationPath).toBe(`m/44'/60'/0'/${i0}/${i1}`);
      expect(i0).toBeLessThan(2 ** 31);
      expect(i1).toBeLessThan(2 ** 31);
      // The unmasked words would have been hardened indices; assert we did not use them.
      expect(derivationPath).not.toContain(String(raw.readUInt32BE(0)));
      expect(derivationPath).not.toContain(String(raw.readUInt32BE(4)));
      // And the derivation actually succeeded rather than throwing on a hardened index.
      expect(address).toMatch(/^0x[0-9a-f]{40}$/);
    });
  });

  describe('output shape', () => {
    it('should return a lowercase address and a matching did:ethr', () => {
      const { did, address, privateKey } = service.derive('shape-check');

      expect(address).toBe(address.toLowerCase());
      expect(address).toMatch(/^0x[0-9a-f]{40}$/);
      expect(did).toBe(`did:ethr:${address}`);
      expect(privateKey).toMatch(/^0x[0-9a-f]{64}$/);
    });

    it('should reject an empty canonicalId', () => {
      expect(() => service.derive('')).toThrow();
      expect(() => service.derive(undefined as unknown as string)).toThrow();
    });
  });

  describe('fail loud on the master seed', () => {
    // An unset or unusable seed must stop the service starting. There is deliberately no
    // fallback seed - a default would mint DIDs nobody can reproduce.
    const badSeeds: Array<[string, any]> = [
      ['missing (undefined)', undefined],
      ['null', null],
      ['empty string', ''],
      ['whitespace only', '   '],
      ['non-hex', 'not-a-hex-seed-value-not-a-hex-seed-value-not-a-hex-seed-value!!'],
      ['odd-length hex', '0'.repeat(63)],
      ['too short (16 bytes)', 'ab'.repeat(16)],
      ['too short (31 bytes)', 'ab'.repeat(31)],
      ['too long (65 bytes)', 'ab'.repeat(65)],
    ];

    it.each(badSeeds)('should throw on init when the seed is %s', async (_label, seed) => {
      const module: TestingModule = await Test.createTestingModule({
        providers: [
          AssetKeyService,
          { provide: ConfigService, useValue: configWith(seed) },
        ],
      }).compile();

      const instance = module.get<AssetKeyService>(AssetKeyService);
      expect(() => instance.onModuleInit()).toThrow(/ASSET_DID_MASTER_SEED/);
    });

    it.each(badSeeds)(
      'should not leak the seed value in the error message when it is %s',
      async (_label, seed) => {
        const module: TestingModule = await Test.createTestingModule({
          providers: [
            AssetKeyService,
            { provide: ConfigService, useValue: configWith(seed) },
          ],
        }).compile();

        const instance = module.get<AssetKeyService>(AssetKeyService);

        let thrown: any;
        try {
          instance.onModuleInit();
        } catch (error) {
          thrown = error;
        }

        expect(thrown).toBeInstanceOf(Error);
        if (typeof seed === 'string' && seed.trim().length > 0) {
          expect(thrown.message).not.toContain(seed);
          expect(thrown.message).not.toContain(seed.trim());
        }
        // A valid seed must never appear either, in case of a copy-paste of this shape.
        expect(thrown.message).not.toContain(TEST_SEED);
      },
    );

    it('should fail the Nest lifecycle, not just the method, when the seed is unset', async () => {
      const module: TestingModule = await Test.createTestingModule({
        providers: [
          AssetKeyService,
          { provide: ConfigService, useValue: configWith(undefined) },
        ],
      }).compile();

      await expect(module.init()).rejects.toThrow(/ASSET_DID_MASTER_SEED/);
    });

    it('should refuse an unimplemented derivation version', async () => {
      await expect(makeService(TEST_SEED, 2)).rejects.toThrow(
        /ASSET_DID_DERIVATION_VERSION/,
      );
    });

    it('should accept a 0x-prefixed seed and derive identically to the bare form', async () => {
      const prefixed = await makeService('0x' + TEST_SEED);
      expect(prefixed.derive(GOLDEN.canonicalId).address).toBe(GOLDEN.address);
    });

    it('should refuse to derive before onModuleInit has run', async () => {
      const module: TestingModule = await Test.createTestingModule({
        providers: [
          AssetKeyService,
          { provide: ConfigService, useValue: configWith(TEST_SEED) },
        ],
      }).compile();

      const uninitialised = module.get<AssetKeyService>(AssetKeyService);
      expect(() => uninitialised.derive(GOLDEN.canonicalId)).toThrow(/not initialised/);
    });
  });
});
