import { ApiProperty } from '@nestjs/swagger';

export class UserInfoDto {
  @ApiProperty({ description: "User's Decentralized Identifier (DID)" })
  did: string;

  @ApiProperty({ description: "Associated GSy DEX Address, if linked", required: false })
  gsyDexAddress?: string;

  @ApiProperty({ description: "Flag indicating if the user has a verified GSY DEX credential", required: false })
  hasVerifiedCredential?: boolean;

  @ApiProperty({ description: "Optional metadata associated with the user", type: 'object', additionalProperties: true, required: false })
  metadata?: Record<string, any>;
}