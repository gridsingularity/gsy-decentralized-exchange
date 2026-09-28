import { ApiProperty } from '@nestjs/swagger';
import { AssetDIDSubjectType } from '../../database/schemas';

/** One entry of the `subjectUuid -> did` map the caller asked for. Never carries key material. */
export class SyncedSubjectDto {
  @ApiProperty({ example: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d' })
  subjectUuid: string;

  @ApiProperty({ enum: AssetDIDSubjectType, example: AssetDIDSubjectType.ASSET })
  subjectType: AssetDIDSubjectType;

  @ApiProperty({ example: 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21' })
  did: string;

  @ApiProperty({
    description: 'Phase 1 writes nothing on-chain, so this is false for every new record.',
    example: false,
  })
  registeredOnChain: boolean;
}

export class AssetSyncResponse {
  @ApiProperty({ description: 'Subjects seen for the first time by this sync', example: 590 })
  created: number;

  @ApiProperty({
    description: 'Subjects already present and re-confirmed by this sync (includes un-retirements)',
    example: 0,
  })
  updated: number;

  @ApiProperty({
    description:
      'Subjects previously known within this payload\'s scope and absent from it, now marked ' +
      'retired. Never deleted.',
    example: 0,
  })
  retired: number;

  @ApiProperty({ type: [SyncedSubjectDto] })
  subjects: SyncedSubjectDto[];
}
