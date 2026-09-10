import { IsString, IsOptional, IsObject } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class DIDUpdateRequest {
  @ApiProperty({
    description: 'Public key to add to the DID document (hex format without 0x prefix)',
    example: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
    required: false,
  })
  @IsOptional()
  @IsString()
  readonly publicKey?: string;

  @ApiProperty({
    description: 'Tag for the public key',
    example: 'key-1',
    required: false,
  })
  @IsOptional()
  @IsString()
  readonly keyTag?: string;

  @ApiProperty({
    description: 'Updated metadata to associate with the DID',
    example: {
      name: 'John Doe',
      organization: 'Updated Corp',
    },
    required: false,
  })
  @IsOptional()
  @IsObject()
  readonly metadata?: Record<string, any>;
}