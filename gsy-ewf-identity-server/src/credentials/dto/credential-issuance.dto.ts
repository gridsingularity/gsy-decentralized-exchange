import { IsString, IsNotEmpty } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class CredentialIssuanceRequest {
  @ApiProperty({
    description: 'DID of the credential subject',
    example: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  @IsString()
  @IsNotEmpty()
  readonly did: string;

  @ApiProperty({
    description: 'Substrate address to link with the DID',
    example: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
  })
  @IsString()
  @IsNotEmpty()
  readonly gsyDexAddress: string;

  @ApiProperty({
    description: 'Challenge signed with the DID private key',
    example: 'Sign this challenge: abcdef1234567890',
  })
  @IsString()
  @IsNotEmpty()
  readonly challenge: string;

  @ApiProperty({
    description: 'Signature created with the DID private key',
    example: '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef',
  })
  @IsString()
  @IsNotEmpty()
  readonly didSignature: string;

  @ApiProperty({
    description: 'Signature created with the Substrate private key',
    example: '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef',
  })
  @IsString()
  @IsNotEmpty()
  readonly substrateSignature: string;
}

export class CredentialIssuanceResponse {
  @ApiProperty({
    description: 'Credential ID',
    example: 'urn:uuid:12345678-1234-1234-1234-123456789012',
  })
  id: string;

  @ApiProperty({
    description: 'Issued verifiable credential in JSON-LD format',
    example: {
      '@context': ['https://www.w3.org/2018/credentials/v1'],
      type: ['VerifiableCredential', 'GSYDexAddressCredential'],
      issuer: 'did:ethr:0x123...',
      issuanceDate: '2023-07-14T12:00:00Z',
      credentialSubject: {
        id: 'did:ethr:0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
        accountLink: {
          gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          chain: 'GSYDex',
        },
      },
      proof: {
        type: 'EcdsaSecp256k1Signature2019',
        created: '2023-07-14T12:00:00Z',
        verificationMethod: 'did:ethr:0x123...#controller',
        proofPurpose: 'assertionMethod',
        jws: '...',
      },
    },
  })
  credential: Record<string, any>;
}