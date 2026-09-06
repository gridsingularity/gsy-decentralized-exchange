import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { Transform } from 'class-transformer';
import { IsBoolean, IsEnum, IsOptional, IsUUID } from 'class-validator';
import { AssetDID, AssetDIDSubjectType } from '../../database/schemas';

/**
 * Read model for `GET /asset-dids` and `GET /asset-dids/:subjectUuid`.
 *
 * Built by an explicit ALLOW-LIST in `fromDocument`, never by spreading the Mongo document.
 * A field added to `AssetDID` later is therefore invisible here until someone adds it on
 * purpose - which is the property that stops a future refactor leaking something it
 * should not. There is no key material to omit today because none is persisted
 * (`asset-did.schema.ts`), and this DTO must stay that way.
 */
export class AssetDIDDto {
  @ApiProperty({ example: 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21' })
  did: string;

  @ApiProperty({ enum: AssetDIDSubjectType })
  subjectType: AssetDIDSubjectType;

  @ApiProperty({ example: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d' })
  subjectUuid: string;

  @ApiProperty({ example: '0xe194ab62fe7f8e28f2266b317bdcfc053999fb21' })
  address: string;

  @ApiProperty({
    description: 'BIP-32 path. Public: it is useless without ASSET_DID_MASTER_SEED.',
    example: "m/44'/60'/0'/2004657372/1277089372",
  })
  derivationPath: string;

  @ApiProperty({ example: 1 })
  derivationVersion: number;

  @ApiProperty({ example: 'Pilot1' })
  communityName: string;

  @ApiProperty({ example: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c' })
  communityUuid: string;

  @ApiPropertyOptional({ example: 'LIC08SM' })
  assetName?: string;

  @ApiPropertyOptional({ example: 'SMART_METER' })
  assetType?: string;

  @ApiPropertyOptional()
  areaHash?: string;

  @ApiProperty({ example: false })
  registeredOnChain: boolean;

  @ApiPropertyOptional()
  registrationTxHash?: string;

  @ApiPropertyOptional()
  registeredAt?: Date;

  @ApiProperty({ example: false })
  retired: boolean;

  @ApiProperty()
  lastSeenAt: Date;

  @ApiPropertyOptional({ type: Object })
  metadata?: Record<string, any>;

  static fromDocument(doc: AssetDID | Record<string, any>): AssetDIDDto {
    const dto = new AssetDIDDto();

    dto.did = doc.did;
    dto.subjectType = doc.subjectType;
    dto.subjectUuid = doc.subjectUuid;
    dto.address = doc.address;
    dto.derivationPath = doc.derivationPath;
    dto.derivationVersion = doc.derivationVersion;
    dto.communityName = doc.communityName;
    dto.communityUuid = doc.communityUuid;
    dto.assetName = doc.assetName;
    dto.assetType = doc.assetType;
    dto.areaHash = doc.areaHash;
    dto.registeredOnChain = doc.registeredOnChain ?? false;
    dto.registrationTxHash = doc.registrationTxHash;
    dto.registeredAt = doc.registeredAt;
    dto.retired = doc.retired ?? false;
    dto.lastSeenAt = doc.lastSeenAt;
    dto.metadata = doc.metadata;

    return dto;
  }
}

/**
 * Turns the string a query string always delivers into a real boolean.
 *
 * `ValidationPipe` runs with `transform: true` but not `enableImplicitConversion`
 * (`main.ts:10-16`), so `?retired=true` arrives as the string `'true'` and a bare
 * `@IsBoolean()` would reject every filter the API documents.
 */
const toOptionalBoolean = ({ value }: { value: unknown }): unknown => {
  if (value === 'true' || value === true) return true;
  if (value === 'false' || value === false) return false;
  return value;
};

/** Query filter for `GET /asset-dids`. Every field optional; an empty query lists everything. */
export class AssetDIDListQuery {
  @ApiPropertyOptional({ example: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c' })
  @IsOptional()
  @IsUUID()
  readonly communityUuid?: string;

  @ApiPropertyOptional({ enum: AssetDIDSubjectType })
  @IsOptional()
  @IsEnum(AssetDIDSubjectType)
  readonly subjectType?: AssetDIDSubjectType;

  @ApiPropertyOptional({ example: false })
  @IsOptional()
  @Transform(toOptionalBoolean)
  @IsBoolean()
  readonly registeredOnChain?: boolean;

  @ApiPropertyOptional({ example: false })
  @IsOptional()
  @Transform(toOptionalBoolean)
  @IsBoolean()
  readonly retired?: boolean;
}
