import { Prop, Schema, SchemaFactory } from '@nestjs/mongoose';
import { Document } from 'mongoose';

/**
 * The two kinds of non-human subject that get a DID. One collection covers both because
 * the derivation, the sync, the auth and (later) the on-chain registration are identical;
 * only the canonical id differs (plan §2.5).
 *
 * The two id spaces provably cannot collide: both are v5 UUIDs in the same namespace, but
 * the preimages are `"Pilot1"` and `"Pilot1:LIC08SM"`, which are never equal.
 */
export enum AssetDIDSubjectType {
  ASSET = 'asset',
  COMMUNITY = 'community',
}

/**
 * A derived, regenerable identity for one ontology asset or one community.
 *
 * Deliberately NOT a discriminator on `User`: `User` is the authenticated-principal type
 * (`AuthService.validateUser`, the passport `'did'` strategy, `verifyChallenge`'s
 * auto-create). Keeping machine subjects in their own collection makes it *structurally*
 * impossible for an asset to become an authenticated principal, rather than relying on a
 * type check being remembered in every present and future guard (plan §2.5).
 *
 * NO PRIVATE KEY IS STORED HERE, AND NONE EVER SHOULD BE.
 * `AssetKeyService.derive()` returns a `privateKey` because a caller needs it to sign
 * (phase 3), but the custody model (plan §2.1) is that keys are *regenerated* from
 * `ASSET_DID_MASTER_SEED` on demand. Mongo is a cache and an index, not the source of
 * truth. Adding a key field here would turn a database leak into a total compromise of
 * every asset identity, and would make a database loss unrecoverable in exactly the way
 * the HD derivation exists to prevent. `test/asset-did.service.spec.ts` asserts that
 * nothing key-shaped reaches this collection.
 */
@Schema({ timestamps: true })
export class AssetDID extends Document {
  /** `did:ethr:<address>`. Unique - two subjects sharing a DID would be an identity swap. */
  @Prop({ required: true, unique: true, index: true })
  did: string;

  @Prop({ required: true, enum: AssetDIDSubjectType, index: true })
  subjectType: AssetDIDSubjectType;

  /**
   * THE canonical id, and the natural join key against `AreaTopologySchema.area_uuid`.
   *   asset     -> deterministic_area_uuid(communityName, assetName)
   *   community -> deterministic_community_uuid(communityName)
   * Computed on the Rust side (`gsy-community-client/.../adapter.rs:62-71`) and never
   * recomputed here - a second source of truth for this key would be a drift hazard for
   * the whole forecast/order/market chain (plan §2.4).
   */
  @Prop({ required: true, unique: true, index: true })
  subjectUuid: string;

  /** Lowercase `0x...`, equal to `did.substring(9)`. Indexed for collision detection. */
  @Prop({ required: true, index: true })
  address: string;

  /** BIP-32 path, `m/44'/60'/0'/i0/i1`. Persisted for cold recovery and audit. */
  @Prop({ required: true })
  derivationPath: string;

  /** Currently always 1. Stamped so a future scheme change is explicit and auditable. */
  @Prop({ required: true })
  derivationVersion: number;

  /** Set on both subject types. Kept for human reconciliation (plan §2.2). */
  @Prop({ required: true })
  communityName: string;

  /** Set on both subject types. On a community record this equals `subjectUuid`. */
  @Prop({ required: true, index: true })
  communityUuid: string;

  /** Assets only. */
  @Prop()
  assetName?: string;

  /** Assets only. */
  @Prop()
  assetType?: string;

  /** Assets only - `deterministic_area_hash(community, assetName)`. */
  @Prop()
  areaHash?: string;

  /**
   * Phase 1 writes nothing on-chain: a `did:ethr` resolves to a valid default DID
   * document with zero registry transactions (plan §2.3). Every record starts false.
   */
  @Prop({ default: false })
  registeredOnChain: boolean;

  /** Phase 3. */
  @Prop()
  registrationTxHash?: string;

  /** Phase 3. */
  @Prop()
  registeredAt?: Date;

  /**
   * True once a `FedecomAssetCredential` has been issued for this subject (phase 4).
   *
   * DELIBERATELY NOT `User.hasVerifiedCredential` (plan §2.7). That flag means "a human
   * principal proved possession of both an Ethereum key and a Substrate account"; this one
   * means "the platform issuer attested about a machine subject that signed nothing". They
   * are different claims with different evidence behind them, and collapsing them would let
   * an asset credential be read as a verified human account.
   *
   * A flag, not a count: the credential collection is the record of what was issued, and
   * `GET /credentials/did/:did` is how you enumerate it. This exists so a sync or an
   * operator can see, in one query over this collection, which subjects still need one.
   */
  @Prop({ default: false })
  hasAssetCredential: boolean;

  /**
   * Records are NEVER deleted. A subject absent from a sync is retired, because a retired
   * DID must stay resolvable: guarantee-of-origin certificates already issued against it
   * have to stay verifiable (plan §2.2).
   */
  @Prop({ default: false })
  retired: boolean;

  /** Bumped for every subject present in a sync payload. */
  @Prop({ required: true, default: Date.now })
  lastSeenAt: Date;

  @Prop({ type: Object })
  metadata?: Record<string, any>;
}

export const AssetDIDSchema = SchemaFactory.createForClass(AssetDID);
