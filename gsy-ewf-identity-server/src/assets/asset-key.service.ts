import { Injectable, OnModuleInit } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { HDNodeWallet } from 'ethers';
import { createHash } from 'crypto';

/**
 * Result of a single derivation. `privateKey` is key material: it is returned here
 * because the caller needs it to sign, and it must not travel anywhere else - never
 * into a log line, an audit record, an HTTP response, or an error message.
 */
export interface DerivedAssetKey {
  did: string;
  address: string;
  privateKey: string;
  derivationPath: string;
  derivationVersion: number;
}

/** Domain separation tag. Part of the derivation contract - changing it changes every DID. */
const DERIVATION_PREFIX = 'gsy-asset-did:v1:';

/** The only derivation scheme implemented by this file. See DERIVATION_PREFIX. */
const SUPPORTED_DERIVATION_VERSION = 1;

/** `HDNodeWallet.fromSeed` accepts 16-64 bytes; plan §6.1 requires at least 32. */
const MIN_SEED_BYTES = 32;
const MAX_SEED_BYTES = 64;

/**
 * Deterministic per-subject key derivation for asset and community DIDs .
 *
 * The whole point of this service is regenerability: master seed + canonical id
 * reproduce the identical DID forever, so Mongo is a cache and not the source of
 * truth. That makes this file a migration surface - once any DID has been minted,
 * changing anything below is a migration, not a refactor.
 *
 * Derivation (plan §2.1, reproduced verbatim):
 *
 *   canonicalId = deterministic_area_uuid(communityName, assetName)  // asset
 *                    or deterministic_community_uuid(communityName)   // community
 *   h                = sha256(utf8("gsy-asset-did:v1:" + canonicalId))
 *   i0               = uint32BE(h[0..4]) & 0x7fffffff
 *   i1               = uint32BE(h[4..8]) & 0x7fffffff
 *   path             = `m/44'/60'/0'/${i0}/${i1}`
 *   wallet           = HDNodeWallet.fromSeed(masterSeed).derivePath(path)
 *   did              = `did:ethr:${wallet.address.toLowerCase()}`
 *
 * `canonicalId` is supplied by the caller and is NEVER recomputed here: the Rust side
 * owns `deterministic_area_uuid` / `deterministic_community_uuid` and re-deriving
 * it in TypeScript would create a second source of truth for a key the whole
 * forecast/order/market chain depends on.
 *
 * SECRET HANDLING: the master seed and every derived private key are never logged
 * (not even at debug level), never stringified into an error, and never returned from
 * anything other than `derive()`.
 */
@Injectable()
export class AssetKeyService implements OnModuleInit {
  /**
   * Root HD node held in memory for the process lifetime. Holding the node rather than
   * the raw seed bytes keeps the seed itself out of any field that could be dumped.
   */
  private rootNode: HDNodeWallet | null = null;

  private derivationVersion: number = SUPPORTED_DERIVATION_VERSION;

  constructor(private readonly configService: ConfigService) {}

  /**
   * Fail loud: a missing, malformed or too-short `ASSET_DID_MASTER_SEED` must stop the
   * service from starting. There is deliberately NO fallback seed - a default would silently
   * mint DIDs that nobody can reproduce, which is worse than not booting.
   *
   * Nest calls this during bootstrap, so a throw here is a boot failure - the same
   * fail-closed shape as `ApiKeyGuard`'s constructor.
   */
  onModuleInit(): void {
    const configured = this.configService.get<string>('assetDid.masterSeed');

    if (typeof configured !== 'string' || configured.trim().length === 0) {
      throw new Error(
        'AssetKeyService: ASSET_DID_MASTER_SEED is not set. Generate one with ' +
          '`openssl rand -hex 32`, store it out of band, and set it in the environment. ' +
          'This service refuses to start rather than derive asset DIDs from a default seed.',
      );
    }

    const normalised = configured.trim().replace(/^0x/i, '');

    if (normalised.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(normalised)) {
      // Deliberately does not echo the value - it is secret key material.
      throw new Error(
        'AssetKeyService: ASSET_DID_MASTER_SEED is not valid hexadecimal. Expected an ' +
          'even-length hex string (optionally 0x-prefixed), e.g. the output of ' +
          '`openssl rand -hex 32`.',
      );
    }

    const seed = Buffer.from(normalised, 'hex');

    if (seed.length < MIN_SEED_BYTES || seed.length > MAX_SEED_BYTES) {
      // The byte length is reported because it is the actionable part of the error;
      // the seed value itself is not.
      throw new Error(
        `AssetKeyService: ASSET_DID_MASTER_SEED must be between ${MIN_SEED_BYTES} and ` +
          `${MAX_SEED_BYTES} bytes, got ${seed.length} bytes. Generate one with ` +
          '`openssl rand -hex 32`.',
      );
    }

    const configuredVersion = this.configService.get<number>('assetDid.derivationVersion');
    const version =
      configuredVersion === undefined || configuredVersion === null
        ? SUPPORTED_DERIVATION_VERSION
        : Number(configuredVersion);

    if (version !== SUPPORTED_DERIVATION_VERSION) {
      // Only v1 exists. Booting with a different number would stamp records with a
      // version whose derivation this file does not implement.
      throw new Error(
        `AssetKeyService: ASSET_DID_DERIVATION_VERSION=${version} is not implemented. ` +
          `Only version ${SUPPORTED_DERIVATION_VERSION} exists; bumping it requires a ` +
          'documented migration, not a config change.',
      );
    }

    this.derivationVersion = version;
    this.rootNode = HDNodeWallet.fromSeed(seed);
    // Zero the local copy of the seed bytes; the root node keeps what it needs.
    seed.fill(0);
  }

  /**
   * Derive the keypair, DID and BIP-32 path for a canonical subject id.
   *
   * @param canonicalId the v5 UUID computed on the Rust side - `deterministic_area_uuid`
   *   for an asset, `deterministic_community_uuid` for a community.
   *   Passed through verbatim; this service never computes it.
   */
  derive(canonicalId: string): DerivedAssetKey {
    if (typeof canonicalId !== 'string' || canonicalId.length === 0) {
      throw new Error('AssetKeyService: canonicalId must be a non-empty string');
    }

    if (this.rootNode === null) {
      throw new Error(
        'AssetKeyService: master seed not initialised - onModuleInit has not run',
      );
    }

    const derivationPath = AssetKeyService.derivationPathFor(canonicalId);
    const wallet = this.rootNode.derivePath(derivationPath);
    const address = wallet.address.toLowerCase();

    return {
      did: `did:ethr:${address}`,
      address,
      privateKey: wallet.privateKey,
      derivationPath,
      derivationVersion: this.derivationVersion,
    };
  }

  /**
   * `m/44'/60'/0'/${i0}/${i1}` from sha256("gsy-asset-did:v1:" + canonicalId).
   *
   * Pure and seed-independent, so it is safe to expose and to test on its own.
   */
  static derivationPathFor(canonicalId: string): string {
    const h = createHash('sha256')
      .update(DERIVATION_PREFIX + canonicalId, 'utf8')
      .digest();

    // The `& 0x7fffffff` mask is LOAD-BEARING, not cosmetic: BIP-32 reserves indices
    // >= 2^31 for hardened derivation, so a non-hardened child index must be < 2^31.
    // Without the mask any digest word with its high bit set would either be rejected
    // by ethers or silently mean a hardened index. Do not "simplify" it away.
    // `>>> 0` keeps readUInt32BE's result unsigned before masking.
    const i0 = (h.readUInt32BE(0) & 0x7fffffff) >>> 0;
    const i1 = (h.readUInt32BE(4) & 0x7fffffff) >>> 0;

    return `m/44'/60'/0'/${i0}/${i1}`;
  }
}
