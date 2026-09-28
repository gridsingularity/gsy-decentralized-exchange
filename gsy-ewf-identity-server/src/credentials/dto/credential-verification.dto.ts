import { IsNotEmpty } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class CredentialVerificationRequest {
  @ApiProperty({
    description: 'Verifiable credential to verify',
    example: {
      '@context': ['https://www.w3.org/2018/credentials/v1'],
      type: ['VerifiableCredential', 'GSYDexAddressCredential'],
      // Other credential properties
    },
  })
  @IsNotEmpty()
  readonly credential: Record<string, any>;
}

export class CredentialVerificationResponse {
  @ApiProperty({
    description: 'Verification result',
    example: true,
  })
  valid: boolean;

  @ApiProperty({
    description: 'DID of the credential subject',
    example: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  did: string;

  @ApiProperty({
    description: 'Linked Substrate address',
    example: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
  })
  gsyDexAddress: string;

  @ApiProperty({
    description: 'Verification details',
    example: {
      issuer: 'valid',
      signature: 'valid',
      expiration: 'valid',
      status: 'active',
    },
  })
  details: Record<string, string>;
}