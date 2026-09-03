import { IsString, IsNotEmpty } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class AuthVerificationRequest {
  @ApiProperty({
    description: 'DID used for authentication',
    example: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  @IsString()
  @IsNotEmpty()
  readonly did: string;

  @ApiProperty({
    description: 'Challenge ID',
    example: '1234567890abcdef',
  })
  @IsString()
  @IsNotEmpty()
  readonly challengeId: string;

  @ApiProperty({
    description: 'Signature of the challenge',
    example: '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef',
  })
  @IsString()
  @IsNotEmpty()
  readonly signature: string;
}

export class AuthVerificationResponse {
  @ApiProperty({
    description: 'JWT access token',
    example: 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...',
  })
  accessToken: string;

  @ApiProperty({
    description: 'DID of the authenticated user',
    example: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  did: string;
}