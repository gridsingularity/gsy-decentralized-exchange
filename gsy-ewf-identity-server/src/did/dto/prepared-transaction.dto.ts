import { ApiProperty } from '@nestjs/swagger';

export class PreparedTransactionDto {
  @ApiProperty({ description: 'The address of the smart contract to interact with (DID Registry).' })
  to: string;

  @ApiProperty({ description: 'The encoded transaction data (including function selector and arguments).' })
  data: string;

  @ApiProperty({ description: 'Optional value field (usually 0 for registry interactions).', default: '0' })
  value?: string; 
}