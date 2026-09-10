import { IsString, IsOptional, IsObject, IsEthereumAddress } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class DIDRequest {
  @ApiProperty({
    description: 'Ethereum address to be used for DID creation',
    example: '0xed6011BBaB3B98cF955ff271F52B12B94BF9fD28',
  })
  @IsString()
  @IsEthereumAddress()
  readonly address: string;

  @ApiProperty({
    description: 'Optional metadata to associate with the DID',
    example: {
      name: 'John Doe',
      organization: 'Example Corp',
    },
    required: false,
  })
  @IsOptional()
  @IsObject()
  readonly metadata?: Record<string, any>;
}