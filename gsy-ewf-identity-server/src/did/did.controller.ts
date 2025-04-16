import {
  Controller, Delete, Get, Post, Patch, Param, Body,
  UseGuards, Req, HttpCode, HttpStatus
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse, ApiBearerAuth, ApiBody } from '@nestjs/swagger'; // Import ApiBody
import { DIDService } from './did.service';
import { DIDRequest } from './dto/did-request.dto';
import { DIDUpdateRequest } from './dto/did-update-request.dto';
import { PreparedTransactionDto } from './dto/prepared-transaction.dto'; // Import DTO
import { DIDAuthGuard } from '../auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../auth/guards/did-owner.guard';

@ApiTags('DID Management')
@Controller('did')
export class DIDController {
  constructor(private readonly didService: DIDService) {}

  @Post()
  @HttpCode(HttpStatus.OK) 
  @ApiOperation({ summary: 'Create local DID record and prepare initial attribute transaction' })
  @ApiResponse({ status: HttpStatus.OK, description: 'Transaction prepared for setting initial DID attribute.', type: PreparedTransactionDto })
  @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input or DID record already exists locally' })
  async createDID(@Body() didRequest: DIDRequest, @Req() req): Promise<PreparedTransactionDto> {
    return this.didService.createDID(didRequest, req);
  }

  @Get(':did')
  @ApiOperation({ summary: 'Resolve a DID document' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID document resolved (may be minimal if no attributes set)' })
  @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'DID not found or not self-owned' })
  async resolveDID(@Param('did') did: string): Promise<any> { 
    return this.didService.resolveDID(did);
  }

  @Post(':did/prepare-update') 
  @UseGuards(DIDAuthGuard, DIDOwnerGuard)
  @ApiBearerAuth()
  @HttpCode(HttpStatus.OK)
  @ApiOperation({ summary: 'Prepare transaction data for updating a DID attribute (e.g., public key)' })
  @ApiBody({ type: DIDUpdateRequest })
  @ApiResponse({ status: HttpStatus.OK, description: 'Transaction prepared for updating DID attribute.', type: PreparedTransactionDto })
  @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input or update not supported.'})
  @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
  @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'DID record not found locally' })
  async prepareUpdateDIDTransaction(
    @Param('did') did: string,
    @Body() updates: DIDUpdateRequest,
    @Req() req
  ): Promise<PreparedTransactionDto> {
    return this.didService.prepareUpdateTransaction(did, updates, req);
  }

  @Post(':did/prepare-deactivate') 
  @UseGuards(DIDAuthGuard, DIDOwnerGuard)
  @ApiBearerAuth()
  @HttpCode(HttpStatus.OK)
  @ApiOperation({ summary: 'Prepare transaction data for deactivating a DID (changing owner to 0x0)' })
  @ApiResponse({ status: HttpStatus.OK, description: 'Transaction prepared for deactivating DID.', type: PreparedTransactionDto })
  @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
  @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'DID record not found locally' })
  async prepareDeactivateDIDTransaction(
    @Param('did') did: string,
    @Req() req
  ): Promise<PreparedTransactionDto> {
    return this.didService.prepareDeactivateTransaction(did, req);
  }

  @Get(':did/exists')
  @ApiOperation({ summary: 'Check if a DID is registered (has attributes/changes on-chain)' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID registration status' })
  async isDIDRegistered(@Param('did') did: string): Promise<{ registered: boolean }> {
    const registered = await this.didService.isDIDRegistered(did);
    return { registered };
  }
}