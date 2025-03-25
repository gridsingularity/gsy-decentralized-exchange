import { 
  Controller, Delete, Get, Post, Patch, Param, Body, 
  UseGuards, Req, HttpCode, HttpStatus 
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse, ApiBearerAuth } from '@nestjs/swagger';
import { DIDService } from './did.service';
import { DIDRequest } from './dto/did-request.dto';
import { DIDUpdateRequest } from './dto/did-update-request.dto';
import { DIDAuthGuard } from '../auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../auth/guards/did-owner.guard';

@ApiTags('DID Management')
@Controller('did')
export class DIDController {
  constructor(private readonly didService: DIDService) {}

  @Post()
  @HttpCode(HttpStatus.CREATED)
  @ApiOperation({ summary: 'Create a new DID' })
  @ApiResponse({ status: HttpStatus.CREATED, description: 'DID created successfully' })
  @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input' })
  async createDID(@Body() didRequest: DIDRequest, @Req() req): Promise<any> {
    return this.didService.createDID(didRequest, req);
  }

  @Get(':did')
  @ApiOperation({ summary: 'Resolve a DID document' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID document resolved' })
  @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'DID not found' })
  async resolveDID(@Param('did') did: string): Promise<any> {
    return this.didService.resolveDID(did);
  }

  @Patch(':did')
  @UseGuards(DIDAuthGuard, DIDOwnerGuard)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Update a DID document' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID document updated' })
  @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
  async updateDID(
    @Param('did') did: string,
    @Body() updates: DIDUpdateRequest,
    @Req() req
  ): Promise<any> {
    return this.didService.updateDID(did, updates, req);
  }

  @Delete(':did')
  @UseGuards(DIDAuthGuard, DIDOwnerGuard)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Deactivate a DID' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID deactivated successfully' })
  @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
  async deactivateDID(
    @Param('did') did: string,
    @Req() req
  ): Promise<{ success: boolean }> {
    const success = await this.didService.deactivateDID(did, req);
    return { success };
  }

  @Get(':did/exists')
  @ApiOperation({ summary: 'Check if a DID is registered' })
  @ApiResponse({ status: HttpStatus.OK, description: 'DID registration status' })
  async isDIDRegistered(@Param('did') did: string): Promise<{ registered: boolean }> {
    const registered = await this.didService.isDIDRegistered(did);
    return { registered };
  }
}