import {
  BadRequestException,
  ConflictException,
  Injectable,
  Logger,
  NotFoundException,
} from '@nestjs/common';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { Request } from 'express';
import { AssetDID, AssetDIDSubjectType, AuditAction } from '../database/schemas';
import { AuditService } from '../audit/audit.service';
import { AssetKeyService } from './asset-key.service';
import {
  AssetSyncItem,
  AssetSyncRequest,
  CommunitySyncItem,
} from './dto/asset-sync-request.dto';
import { AssetSyncResponse, SyncedSubjectDto } from './dto/asset-sync-response.dto';
import { AssetDIDDto, AssetDIDListQuery } from './dto/asset-did.dto';

/** A payload item flattened to the shape the upsert works on, before derivation. */
interface NormalisedSubject {
  subjectUuid: string;
  subjectType: AssetDIDSubjectType;
  communityName: string;
  communityUuid: string;
  assetName?: string;
  assetType?: string;
  areaHash?: string;
}

/** A normalised subject plus its derived identity. Never carries the private key. */
interface DerivedSubject extends NormalisedSubject {
  did: string;
  address: string;
  derivationPath: string;
  derivationVersion: number;
}

/**
 * How many audit inserts to have in flight at once on a first sync.
 *
 * Only the first sync of a community writes audit rows at all (creates and retirements
 * only, never plain updates), so this bounds a one-off burst of ~590 inserts rather than
 * anything steady-state. Kept well under Mongoose's default pool size of 100.
 */
const AUDIT_CONCURRENCY = 25;

/**
 * Owns the `AssetDID` collection: bulk idempotent sync, and lookup.
 *
 * Two invariants run through the whole file.
 *
 * 1. NOTHING KEY-SHAPED IS PERSISTED, RETURNED OR LOGGED. `AssetKeyService.derive()`
 *    returns a `privateKey` because phase 3 needs it to sign; this service destructures
 *    the four public fields it wants and drops the rest on the floor. Never spread a
 *    `DerivedAssetKey` into a document, an audit record or a response - the custody model
 *    (plan §2.1) is that keys are regenerated from the master seed, never stored.
 * 2. RECORDS ARE NEVER DELETED. A subject that disappears from the ontology is retired,
 *    because guarantee-of-origin certificates already issued against its DID must stay
 *    verifiable (plan §2.2).
 */
@Injectable()
export class AssetDIDService {
  private readonly logger = new Logger(AssetDIDService.name);

  constructor(
    @InjectModel(AssetDID.name) private readonly assetDidModel: Model<AssetDID>,
    private readonly assetKeyService: AssetKeyService,
    private readonly auditService: AuditService,
  ) {}

  /**
   * Idempotent bulk upsert of communities and assets, plus scoped retirement.
   *
   * Posting the identical payload twice yields `created: 0` and byte-identical DIDs: the
   * DID is a pure function of `(masterSeed, subjectUuid)`, so a re-sync re-derives rather
   * than re-mints. That is also why a re-appearing retired subject keeps its DID.
   */
  async syncSubjects(dto: AssetSyncRequest, request?: Request): Promise<AssetSyncResponse> {
    const communities = dto?.communities ?? [];
    const assets = dto?.assets ?? [];

    // Also enforced by AssetSyncPayloadNotEmptyConstraint at the DTO layer; repeated here
    // because the service is called directly (tests, and any future internal caller).
    if (communities.length === 0 && assets.length === 0) {
      throw new BadRequestException(
        'a sync payload must carry at least one community or one asset',
      );
    }

    const subjects = AssetDIDService.normalise(communities, assets);
    AssetDIDService.assertNoDuplicateSubjectUuid(subjects);

    const derived = subjects.map((subject) => this.deriveFor(subject));
    AssetDIDService.assertNoIntraPayloadAddressCollision(derived);

    const subjectUuids = derived.map((d) => d.subjectUuid);
    const addresses = derived.map((d) => d.address);

    // ONE round trip for the whole payload rather than 590. Everything already known
    // under either its subjectUuid or its derived address comes back in a single query,
    // which is also what makes the cross-subject collision check cheap.
    const existing: AssetDID[] = await this.assetDidModel
      .find({ $or: [{ subjectUuid: { $in: subjectUuids } }, { address: { $in: addresses } }] })
      .lean()
      .exec();

    const bySubjectUuid = new Map<string, AssetDID>();
    const byAddress = new Map<string, AssetDID>();
    for (const doc of existing ?? []) {
      bySubjectUuid.set(doc.subjectUuid, doc);
      byAddress.set(doc.address, doc);
    }

    AssetDIDService.assertNoStoredAddressCollision(derived, byAddress);

    const now = new Date();
    const operations: any[] = [];
    const createdSubjects: DerivedSubject[] = [];
    const responseSubjects: SyncedSubjectDto[] = [];
    let updated = 0;

    for (const subject of derived) {
      const stored = bySubjectUuid.get(subject.subjectUuid);

      if (!stored) {
        createdSubjects.push(subject);
        operations.push({
          insertOne: {
            document: {
              did: subject.did,
              subjectType: subject.subjectType,
              subjectUuid: subject.subjectUuid,
              address: subject.address,
              derivationPath: subject.derivationPath,
              derivationVersion: subject.derivationVersion,
              communityName: subject.communityName,
              communityUuid: subject.communityUuid,
              ...AssetDIDService.assetOnlyFields(subject),
              registeredOnChain: false,
              retired: false,
              lastSeenAt: now,
            },
          },
        });
        responseSubjects.push({
          subjectUuid: subject.subjectUuid,
          subjectType: subject.subjectType,
          did: subject.did,
          registeredOnChain: false,
        });
        continue;
      }

      updated += 1;
      operations.push({
        updateOne: {
          filter: { subjectUuid: subject.subjectUuid },
          update: {
            $set: {
              // Re-derived, not re-minted: for an unchanged subjectUuid these are
              // byte-identical to what is stored. Written anyway so a record whose
              // derivation was interrupted converges.
              did: subject.did,
              address: subject.address,
              derivationPath: subject.derivationPath,
              derivationVersion: subject.derivationVersion,
              communityName: subject.communityName,
              communityUuid: subject.communityUuid,
              ...AssetDIDService.assetOnlyFields(subject),
              // Un-retire. A subject that comes back keeps the DID it always had.
              retired: false,
              lastSeenAt: now,
            },
          },
        },
      });
      responseSubjects.push({
        subjectUuid: subject.subjectUuid,
        subjectType: subject.subjectType,
        did: subject.did,
        registeredOnChain: stored.registeredOnChain ?? false,
      });
    }

    const toRetire = await this.findRetirementCandidates(communities, assets, subjectUuids);
    for (const doc of toRetire) {
      operations.push({
        updateOne: {
          filter: { subjectUuid: doc.subjectUuid },
          update: { $set: { retired: true } },
        },
      });
    }

    if (operations.length > 0) {
      // One ordered:false bulkWrite for creates + updates + retirements. The per-subject
      // create/update/retire classification is decided above, from the single `find`, so
      // batching costs nothing in the counts the response has to report.
      await this.assetDidModel.bulkWrite(operations, { ordered: false });
    }

    await this.auditCreatedAndRetired(createdSubjects, toRetire, request);

    this.logger.log(
      `asset-did sync: created=${createdSubjects.length} updated=${updated} ` +
        `retired=${toRetire.length} subjects=${derived.length}`,
    );

    return {
      created: createdSubjects.length,
      updated,
      retired: toRetire.length,
      subjects: responseSubjects,
    };
  }

  /** Single record by canonical id. 404 only if the subject was never seen. */
  async findBySubjectUuid(subjectUuid: string): Promise<AssetDIDDto> {
    const doc = await this.assetDidModel.findOne({ subjectUuid }).lean().exec();

    if (!doc) {
      throw new NotFoundException(`No DID record for subject ${subjectUuid}`);
    }

    // Retired records are returned deliberately: their DIDs must stay resolvable for
    // certificates already issued against them (plan §2.2). `retired` is on the DTO.
    return AssetDIDDto.fromDocument(doc);
  }

  /** Filtered list. An absent filter field is simply not part of the query. */
  async list(filter: AssetDIDListQuery = {}): Promise<AssetDIDDto[]> {
    const query: Record<string, any> = {};

    if (filter.communityUuid !== undefined) query.communityUuid = filter.communityUuid;
    if (filter.subjectType !== undefined) query.subjectType = filter.subjectType;
    if (filter.registeredOnChain !== undefined) query.registeredOnChain = filter.registeredOnChain;
    if (filter.retired !== undefined) query.retired = filter.retired;

    const docs = await this.assetDidModel.find(query).sort({ subjectUuid: 1 }).lean().exec();

    return (docs ?? []).map((doc) => AssetDIDDto.fromDocument(doc));
  }

  /**
   * Which stored subjects this payload is entitled to retire.
   *
   * SCOPE CHOICE: `(subjectType, communityUuid)` pairs actually present in the payload.
   * A sync that carries only Pilot1 must not retire Pilot2's assets, so a global
   * "everything not in the payload" sweep is wrong - the community client posts per its
   * own view of the world and a partial post must not look like a mass deletion. Scoping
   * by community alone is also wrong, because both arrays are independently optional: an
   * assets-only sync would then retire the community records of the very communities it
   * just confirmed. Pairing the type with the community makes each half of the payload
   * authoritative only over its own kind of subject.
   *
   * The cost is that a community whose assets vanish entirely cannot be emptied by a sync
   * that omits it - it simply falls out of scope. That is the safe direction to fail: it
   * leaves DIDs resolvable, which is the invariant that matters.
   */
  private async findRetirementCandidates(
    communities: CommunitySyncItem[],
    assets: AssetSyncItem[],
    presentSubjectUuids: string[],
  ): Promise<AssetDID[]> {
    const scope: Record<string, any>[] = [];

    const assetCommunityUuids = AssetDIDService.distinct(assets.map((a) => a.communityUuid));
    if (assetCommunityUuids.length > 0) {
      scope.push({
        subjectType: AssetDIDSubjectType.ASSET,
        communityUuid: { $in: assetCommunityUuids },
      });
    }

    const communityUuids = AssetDIDService.distinct(communities.map((c) => c.communityUuid));
    if (communityUuids.length > 0) {
      scope.push({
        subjectType: AssetDIDSubjectType.COMMUNITY,
        communityUuid: { $in: communityUuids },
      });
    }

    if (scope.length === 0) {
      return [];
    }

    // `retired: false` keeps this to subjects that are *newly* retired, so the audit trail
    // does not repeat the same retirement on every sync interval.
    return this.assetDidModel
      .find({
        $or: scope,
        subjectUuid: { $nin: presentSubjectUuids },
        retired: false,
      })
      .lean()
      .exec();
  }

  /**
   * Audit creates and newly-retired subjects, and nothing else.
   *
   * Deliberately NOT logging plain updates: 590 subjects re-confirmed hourly would write
   * ~14k audit rows a day that say nothing happened, and would bury the events that do
   * matter.
   */
  private async auditCreatedAndRetired(
    created: DerivedSubject[],
    retired: AssetDID[],
    request?: Request,
  ): Promise<void> {
    const entries: Array<{ action: AuditAction; did: string; metadata: Record<string, any> }> = [];

    for (const subject of created) {
      entries.push({
        action: AuditAction.ASSET_DID_CREATED,
        did: subject.did,
        // Allow-listed metadata. Never the derived key, and never a whole DerivedAssetKey.
        metadata: {
          subjectUuid: subject.subjectUuid,
          subjectType: subject.subjectType,
          communityName: subject.communityName,
          communityUuid: subject.communityUuid,
          assetName: subject.assetName,
          assetType: subject.assetType,
          derivationVersion: subject.derivationVersion,
        },
      });
    }

    for (const doc of retired) {
      entries.push({
        action: AuditAction.ASSET_DID_RETIRED,
        did: doc.did,
        metadata: {
          subjectUuid: doc.subjectUuid,
          subjectType: doc.subjectType,
          communityUuid: doc.communityUuid,
          reason: 'absent from sync payload',
        },
      });
    }

    for (let i = 0; i < entries.length; i += AUDIT_CONCURRENCY) {
      const batch = entries.slice(i, i + AUDIT_CONCURRENCY);
      await Promise.all(
        batch.map((entry) =>
          this.auditService.log(entry.action, entry.did, request, entry.metadata),
        ),
      );
    }
  }

  /**
   * Derive and immediately discard the private key.
   *
   * The explicit field list is the point: `return {...subject, ...this.assetKeyService
   * .derive(...)}` would be shorter and would silently carry `privateKey` into every
   * document and audit record downstream.
   */
  private deriveFor(subject: NormalisedSubject): DerivedSubject {
    const { did, address, derivationPath, derivationVersion } = this.assetKeyService.derive(
      subject.subjectUuid,
    );

    return { ...subject, did, address, derivationPath, derivationVersion };
  }

  private static normalise(
    communities: CommunitySyncItem[],
    assets: AssetSyncItem[],
  ): NormalisedSubject[] {
    const fromCommunities = communities.map((c) => ({
      subjectUuid: c.subjectUuid,
      subjectType: AssetDIDSubjectType.COMMUNITY,
      communityName: c.communityName,
      communityUuid: c.communityUuid,
    }));

    const fromAssets = assets.map((a) => ({
      subjectUuid: a.subjectUuid,
      subjectType: AssetDIDSubjectType.ASSET,
      communityName: a.communityName,
      communityUuid: a.communityUuid,
      assetName: a.assetName,
      assetType: a.assetType,
      areaHash: a.areaHash,
    }));

    return [...fromCommunities, ...fromAssets];
  }

  /** Asset-only fields are absent, not null, on a community record (plan §2.5). */
  private static assetOnlyFields(subject: NormalisedSubject): Record<string, string> {
    if (subject.subjectType !== AssetDIDSubjectType.ASSET) {
      return {};
    }

    return {
      assetName: subject.assetName,
      assetType: subject.assetType,
      areaHash: subject.areaHash,
    };
  }

  private static assertNoDuplicateSubjectUuid(subjects: NormalisedSubject[]): void {
    const seen = new Set<string>();

    for (const subject of subjects) {
      if (seen.has(subject.subjectUuid)) {
        // `subjectUuid` is unique in the collection, so a duplicated one would make the
        // bulkWrite's outcome depend on operation order. Reject the payload instead.
        throw new BadRequestException(
          `Duplicate subjectUuid in sync payload: ${subject.subjectUuid}`,
        );
      }
      seen.add(subject.subjectUuid);
    }
  }

  /**
   * Collision detection, half one: two subjects in this payload deriving the same address.
   * ~10^-13 at 590 subjects (plan §2.2) - but silently letting one overwrite the other is
   * an identity swap, so fail the whole sync loudly instead.
   */
  private static assertNoIntraPayloadAddressCollision(derived: DerivedSubject[]): void {
    const byAddress = new Map<string, string>();

    for (const subject of derived) {
      const other = byAddress.get(subject.address);

      if (other !== undefined && other !== subject.subjectUuid) {
        throw new ConflictException(
          `Derivation collision: subjects ${other} and ${subject.subjectUuid} derive the ` +
            `same address ${subject.address}. Sync aborted; no record was written.`,
        );
      }
      byAddress.set(subject.address, subject.subjectUuid);
    }
  }

  /**
   * Collision detection, half two: a derived address already stored against a *different*
   * subject. Checked before any write, so an aborted sync leaves the collection untouched.
   */
  private static assertNoStoredAddressCollision(
    derived: DerivedSubject[],
    byAddress: Map<string, AssetDID>,
  ): void {
    for (const subject of derived) {
      const stored = byAddress.get(subject.address);

      if (stored !== undefined && stored.subjectUuid !== subject.subjectUuid) {
        throw new ConflictException(
          `Derivation collision: address ${subject.address} derived for subject ` +
            `${subject.subjectUuid} is already held by subject ${stored.subjectUuid}. ` +
            'Sync aborted; no record was written.',
        );
      }
    }
  }

  private static distinct(values: string[]): string[] {
    return Array.from(new Set(values));
  }
}
