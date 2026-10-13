import { IsString } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class AuthChallengeRequest {
  @ApiProperty({
    description: 'DID to generate challenge for',
    example: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  @IsString()
  readonly did: string;
}

export class AuthChallengeResponse {
  @ApiProperty({
    description: 'Challenge ID',
    example: '1234567890abcdef',
  })
  id: string;

  @ApiProperty({
    description: 'Challenge to be signed',
    example: 'Sign this message to authenticate: 1234567890abcdef',
  })
  challenge: string;

  @ApiProperty({
    description: 'Timestamp of challenge creation',
    example: '2023-07-14T12:00:00.000Z',
  })
  timestamp: Date;
}