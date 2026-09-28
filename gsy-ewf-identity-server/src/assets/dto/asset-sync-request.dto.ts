import {
  IsArray,
  IsNotEmpty,
  IsOptional,
  IsString,
  IsUUID,
  Validate,
  ValidateNested,
  ValidatorConstraint,
  ValidatorConstraintInterface,
} from 'class-validator';
import { Type } from 'class-transformer';
import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';

/**
 * One community subject. Its canonical id is `deterministic_community_uuid(communityName)`,
 * so `subjectUuid` and `communityUuid` are the same value on this item - both are carried
 * because the sync treats the two subject types uniformly and `communityUuid` is what the
 * retirement scope and the `GET /asset-dids` filter key on.
 */
export class CommunitySyncItem {
  @ApiProperty({
    description:
      'Canonical subject id - deterministic_community_uuid(communityName), computed by ' +
      'gsy-community-client (adapter.rs:62-64). Never recomputed server-side.',
    example: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
  })
  @IsUUID()
  readonly subjectUuid: string;

  @ApiProperty({ description: 'Ontology LEC name', example: 'Pilot1' })
  @IsString()
  @IsNotEmpty()
  readonly communityName: string;

  @ApiProperty({
    description: 'deterministic_community_uuid(communityName). Equals subjectUuid here.',
    example: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
  })
  @IsUUID()
  readonly communityUuid: string;
}

/** One asset subject. Canonical id is `deterministic_area_uuid(communityName, assetName)`. */
export class AssetSyncItem {
  @ApiProperty({
    description:
      'Canonical subject id - deterministic_area_uuid(communityName, assetName), computed ' +
      'by gsy-community-client (adapter.rs:68-71). Never recomputed server-side.',
    example: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d',
  })
  @IsUUID()
  readonly subjectUuid: string;

  @ApiProperty({ description: 'Ontology asset name', example: 'LIC08SM' })
  @IsString()
  @IsNotEmpty()
  readonly assetName: string;

  @ApiProperty({
    description: 'Mapped AssetType from gsy-community-client (topology.rs:107-138)',
    example: 'SMART_METER',
  })
  @IsString()
  @IsNotEmpty()
  readonly assetType: string;

  @ApiProperty({ description: 'Ontology LEC name', example: 'Pilot1' })
  @IsString()
  @IsNotEmpty()
  readonly communityName: string;

  @ApiProperty({
    description: 'deterministic_community_uuid(communityName)',
    example: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
  })
  @IsUUID()
  readonly communityUuid: string;

  @ApiProperty({
    description: 'deterministic_area_hash(communityName, assetName), blake2b-256 hex',
    example: '0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0',
  })
  @IsString()
  @IsNotEmpty()
  readonly areaHash: string;
}

/**
 * Rejects a payload carrying neither communities nor assets.
 *
 * Attached to the `subjectCount` getter rather than to `assets`/`communities`: both arrays
 * are independently optional, and `@IsOptional()` on a property short-circuits *every*
 * validator on that property, so a cross-field check anchored on either array would never
 * run in exactly the case it exists to catch (both absent).
 */
@ValidatorConstraint({ name: 'assetSyncPayloadNotEmpty', async: false })
export class AssetSyncPayloadNotEmptyConstraint implements ValidatorConstraintInterface {
  validate(value: unknown): boolean {
    return typeof value === 'number' && value > 0;
  }

  defaultMessage(): string {
    return 'a sync payload must carry at least one community or one asset';
  }
}

/**
 * Bulk, idempotent sync payload: the whole set of subjects the caller currently knows about
 * for the communities it covers.
 *
 * `main.ts:10-16` sets `forbidNonWhitelisted: true`, so any field a caller sends that is not
 * declared here produces a 400. That cuts both ways: it is also why `@ValidateNested` +
 * `@Type` are mandatory below. Without `@Type` class-transformer leaves the array elements
 * as plain objects, class-validator finds no metadata on them, and every nested item is
 * silently accepted - including one with a malformed `subjectUuid`, which would then be
 * used verbatim as a key-derivation input.
 */
export class AssetSyncRequest {
  @ApiPropertyOptional({ type: [CommunitySyncItem] })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => CommunitySyncItem)
  readonly communities?: CommunitySyncItem[];

  @ApiPropertyOptional({ type: [AssetSyncItem] })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => AssetSyncItem)
  readonly assets?: AssetSyncItem[];

  /**
   * Validation anchor only - not a wire field. It lives on the prototype, so it is neither
   * stripped by `whitelist` nor rejected by `forbidNonWhitelisted` (both of which act on
   * the instance's own keys), but class-validator still reads it and runs the constraint.
   */
  @Validate(AssetSyncPayloadNotEmptyConstraint)
  get subjectCount(): number {
    return (this.communities?.length ?? 0) + (this.assets?.length ?? 0);
  }

  /**
   * Discards any `subjectCount` a caller happens to send. class-transformer copies every
   * key of the plain body onto the instance, and assigning to a getter-only accessor
   * throws a TypeError (class bodies are strict mode) - which would surface as a 500
   * instead of a 400. Swallowing the value keeps `subjectCount` derived from the arrays.
   */
  set subjectCount(_ignored: number) {
    /* intentionally empty */
  }
}
