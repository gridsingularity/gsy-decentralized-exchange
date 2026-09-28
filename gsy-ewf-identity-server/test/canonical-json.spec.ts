import { canonicalize, CanonicalizationError } from '../src/common/canonical-json';

/**
 * Regression suite for plan §0.5 bug (B).
 *
 * The two properties that matter are independent, and only one of them is obvious:
 *  - determinism, so issue and verify agree (defect 1);
 *  - LOSSLESSNESS, so the signature actually covers the claims (defect 2).
 * A canonicaliser can have the first without the second - that is exactly what the old
 * `JSON.stringify(x, Object.keys(x).sort())` form was - so both are asserted here.
 */
describe('canonicalize', () => {
  /** The exact shape the service signs, minus the proof. */
  const gsyDexCredential = {
    '@context': ['https://www.w3.org/2018/credentials/v1'],
    id: 'urn:uuid:11111111-2222-4333-8444-555555555555',
    type: ['VerifiableCredential', 'GSYDexAddressCredential'],
    issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
    issuanceDate: '2026-09-06T00:00:00.000Z',
    expirationDate: '2027-09-06T00:00:00.000Z',
    credentialSubject: {
      id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
      accountLink: {
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        chain: 'GSYDex',
      },
    },
  };

  describe('key order independence', () => {
    it('is unaffected by top-level key order', () => {
      const a = { alpha: 1, beta: 2, gamma: 3 };
      const b = { gamma: 3, alpha: 1, beta: 2 };

      expect(canonicalize(a)).toBe(canonicalize(b));
      expect(canonicalize(a)).toBe('{"alpha":1,"beta":2,"gamma":3}');
    });

    it('is unaffected by key order at EVERY nesting depth', () => {
      // Four levels deep, every level shuffled differently. Sorting only the top level
      // (or only two levels) passes the previous test and fails this one.
      const a = { l1: { l2: { l3: { z: 1, a: 2 }, y: 3 }, x: 4 }, w: 5 };
      const b = { w: 5, l1: { x: 4, l2: { y: 3, l3: { a: 2, z: 1 } } } };

      expect(canonicalize(a)).toBe(canonicalize(b));
      expect(canonicalize(a)).toBe('{"l1":{"l2":{"l3":{"a":2,"z":1},"y":3},"x":4},"w":5}');
    });

    it('is unaffected by key order inside objects nested in arrays', () => {
      const a = { items: [{ b: 1, a: 2 }, { d: 3, c: 4 }] };
      const b = { items: [{ a: 2, b: 1 }, { c: 4, d: 3 }] };

      expect(canonicalize(a)).toBe(canonicalize(b));
    });

    it('produces identical bytes for a credential that has been through JSON transport', () => {
      // What actually happens in production: Mongo and Express both hand back an object
      // whose key order need not match the literal the issuer signed.
      const reordered = {
        credentialSubject: {
          accountLink: {
            chain: 'GSYDex',
            gsyDexAddress: gsyDexCredential.credentialSubject.accountLink.gsyDexAddress,
          },
          id: gsyDexCredential.credentialSubject.id,
        },
        expirationDate: gsyDexCredential.expirationDate,
        issuanceDate: gsyDexCredential.issuanceDate,
        issuer: gsyDexCredential.issuer,
        type: gsyDexCredential.type,
        id: gsyDexCredential.id,
        '@context': gsyDexCredential['@context'],
      };

      expect(canonicalize(reordered)).toBe(canonicalize(gsyDexCredential));
    });
  });

  describe('nested claims survive (regression for §0.5 B defect 2)', () => {
    /**
     * THE test this file exists for.
     *
     * The old form, `JSON.stringify(v, Object.keys(v).sort())`, applies its key list at
     * every nesting level, so `gsyDexAddress` - which is not a top-level key - is deleted
     * before signing. Asserting `toContain('gsyDexAddress')` is what fails if anyone
     * "simplifies" `canonicalize` back to a replacer.
     */
    it('keeps deeply nested claim values', () => {
      const output = canonicalize(gsyDexCredential);

      expect(output).toContain('gsyDexAddress');
      expect(output).toContain('5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN');
      expect(output).toContain('GSYDex');
    });

    it('keeps every field of a FedecomAssetCredential subject', () => {
      // Every one of these is a nested key with no top-level namesake, so every one of
      // them vanished under the replacer form - leaving a signature over the DID alone.
      const assetCredential = {
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        id: 'urn:uuid:99999999-2222-4333-8444-555555555555',
        type: ['VerifiableCredential', 'FedecomAssetCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: '2026-09-06T00:00:00.000Z',
        expirationDate: '2027-09-06T00:00:00.000Z',
        credentialSubject: {
          id: 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21',
          subjectUuid: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d',
          assetName: 'LIC08SM',
          assetType: 'SMART_METER',
          communityName: 'Pilot1',
          communityUuid: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
        },
      };

      const output = canonicalize(assetCredential);

      for (const claim of ['subjectUuid', 'assetName', 'assetType', 'communityName', 'communityUuid']) {
        expect(output).toContain(claim);
      }
      expect(output).toContain('LIC08SM');
      expect(output).toContain('Pilot1');
    });

    it('demonstrates the old replacer form losing the claim it was meant to sign', () => {
      // Not a test of production code - a pinned demonstration of why this module exists.
      // If this ever stops holding, the premise of the whole fix has changed.
      const replacerForm = JSON.stringify(
        gsyDexCredential,
        Object.keys(gsyDexCredential).sort(),
      );

      expect(replacerForm).not.toContain('gsyDexAddress');
      expect(canonicalize(gsyDexCredential)).toContain('gsyDexAddress');
    });
  });

  describe('array order', () => {
    it('never sorts arrays', () => {
      // `type[0]` must stay 'VerifiableCredential' per the W3C VC data model, and
      // alphabetical order would put 'FedecomAssetCredential' first.
      const output = canonicalize({ type: ['VerifiableCredential', 'FedecomAssetCredential'] });

      expect(output).toBe('{"type":["VerifiableCredential","FedecomAssetCredential"]}');
    });

    it('distinguishes two arrays that differ only in order', () => {
      expect(canonicalize([3, 1, 2])).not.toBe(canonicalize([1, 2, 3]));
      expect(canonicalize([3, 1, 2])).toBe('[3,1,2]');
    });

    it('preserves order of nested arrays and of objects inside them', () => {
      const value = { a: [['z', 'y'], ['b', 'a']] };

      expect(canonicalize(value)).toBe('{"a":[["z","y"],["b","a"]]}');
    });
  });

  describe('idempotence', () => {
    it('round-trips through JSON.parse unchanged', () => {
      const once = canonicalize(gsyDexCredential);

      expect(canonicalize(JSON.parse(once))).toBe(once);
    });

    it('round-trips a shuffled nested structure unchanged', () => {
      const value = { z: { b: [1, { d: 4, c: 3 }], a: null }, y: 'x' };
      const once = canonicalize(value);

      expect(canonicalize(JSON.parse(once))).toBe(once);
      expect(canonicalize(JSON.parse(canonicalize(JSON.parse(once))))).toBe(once);
    });
  });

  describe('scalars and edge shapes', () => {
    it('serialises primitives the way JSON does', () => {
      expect(canonicalize(null)).toBe('null');
      expect(canonicalize(true)).toBe('true');
      expect(canonicalize(false)).toBe('false');
      expect(canonicalize(42)).toBe('42');
      expect(canonicalize(-1.5)).toBe('-1.5');
      expect(canonicalize('hi')).toBe('"hi"');
      expect(canonicalize({})).toBe('{}');
      expect(canonicalize([])).toBe('[]');
    });

    it('escapes strings and keys', () => {
      expect(canonicalize({ 'a"b': 'c\nd' })).toBe('{"a\\"b":"c\\nd"}');
    });

    it('keeps null values rather than dropping them', () => {
      expect(canonicalize({ b: null, a: 1 })).toBe('{"a":1,"b":null}');
    });

    it('honours toJSON, like JSON.stringify', () => {
      const withDate = { when: new Date('2026-09-06T00:00:00.000Z'), a: 1 };

      expect(canonicalize(withDate)).toBe('{"a":1,"when":"2026-09-06T00:00:00.000Z"}');
    });

    it('sorts numeric-looking keys by code unit, not numerically', () => {
      // Object.keys returns integer-like keys first in ascending numeric order; the sort
      // must not depend on that quirk, in either direction.
      expect(canonicalize({ 10: 'a', 9: 'b', 2: 'c' })).toBe('{"10":"a","2":"c","9":"b"}');
    });
  });

  describe('rejects what JSON.stringify would silently lose', () => {
    it('rejects a bare undefined', () => {
      expect(() => canonicalize(undefined)).toThrow(CanonicalizationError);
    });

    it('rejects an undefined property rather than dropping it', () => {
      // JSON.stringify would emit '{"a":1}' and the missing claim would go unsigned.
      expect(() => canonicalize({ a: 1, b: undefined })).toThrow(CanonicalizationError);
    });

    it('rejects an undefined nested deep inside a claim', () => {
      expect(() =>
        canonicalize({ credentialSubject: { accountLink: { gsyDexAddress: undefined } } }),
      ).toThrow(/credentialSubject\.accountLink\.gsyDexAddress/);
    });

    it('rejects an undefined inside an array rather than nulling it', () => {
      expect(() => canonicalize({ a: [1, undefined] })).toThrow(CanonicalizationError);
    });

    it('rejects a direct cycle', () => {
      const value: Record<string, unknown> = { a: 1 };
      value.self = value;

      expect(() => canonicalize(value)).toThrow(CanonicalizationError);
      expect(() => canonicalize(value)).toThrow(/circular/);
    });

    it('rejects an indirect cycle through an array', () => {
      const inner: Record<string, unknown> = {};
      const outer = { list: [inner] };
      inner.back = outer;

      expect(() => canonicalize(outer)).toThrow(/circular/);
    });

    it('accepts the same object appearing twice as siblings (not a cycle)', () => {
      const shared = { a: 1 };

      expect(canonicalize({ x: shared, y: shared })).toBe('{"x":{"a":1},"y":{"a":1}}');
    });

    it('rejects NaN and Infinity rather than emitting null', () => {
      expect(() => canonicalize({ a: NaN })).toThrow(CanonicalizationError);
      expect(() => canonicalize({ a: Infinity })).toThrow(CanonicalizationError);
    });

    it('rejects functions and symbols rather than dropping them', () => {
      expect(() => canonicalize({ a: () => 1 })).toThrow(CanonicalizationError);
      expect(() => canonicalize({ a: Symbol('s') })).toThrow(CanonicalizationError);
    });

    it('rejects a bigint rather than throwing an opaque TypeError', () => {
      expect(() => canonicalize({ a: BigInt(1) })).toThrow(CanonicalizationError);
    });
  });
});
