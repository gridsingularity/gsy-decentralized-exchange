import { 
    Controller, Post, Get, Delete, Body, Param, 
    UseGuards, Req, HttpCode, HttpStatus, ForbiddenException,
    NotFoundException
  } from '@nestjs/common';
  import { ApiTags, ApiOperation, ApiResponse, ApiBearerAuth } from '@nestjs/swagger';
  import { CredentialsService } from './credentials.service';
  import { 
    CredentialIssuanceRequest, 
    CredentialIssuanceResponse 
  } from './dto/credential-issuance.dto';
  import { 
    CredentialVerificationRequest, 
    CredentialVerificationResponse 
  } from './dto/credential-verification.dto';
  import { DIDAuthGuard } from '../auth/guards/did-auth.guard';
  import { DIDOwnerGuard } from '../auth/guards/did-owner.guard';

  @ApiTags('Credentials')
  @Controller('credentials')
  export class CredentialsController {
    constructor(private readonly credentialsService: CredentialsService) {}
  
    @Post('issue')
    @HttpCode(HttpStatus.CREATED)
    @ApiOperation({ summary: 'Issue a credential linking DID to Substrate address' })
    @ApiResponse({ status: HttpStatus.CREATED, description: 'Credential issued successfully', type: CredentialIssuanceResponse })
    @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input' })
    @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Invalid signatures' })
    async issueCredential(
      @Body() request: CredentialIssuanceRequest,
      @Req() req,
    ): Promise<CredentialIssuanceResponse> {
      return this.credentialsService.issueCredential(
        request.did,
        request.gsyDexAddress,
        request.challenge,
        request.didSignature,
        request.substrateSignature,
        req,
      );
    }
  
    @Post('verify')
    @HttpCode(HttpStatus.OK)
    @ApiOperation({ summary: 'Verify a credential' })
    @ApiResponse({ status: HttpStatus.OK, description: 'Credential verification result', type: CredentialVerificationResponse })
    @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input' })
    async verifyCredential(
      @Body() request: CredentialVerificationRequest,
      @Req() req,
    ): Promise<CredentialVerificationResponse> {
      return this.credentialsService.verifyCredential(request.credential, req);
    }
  
    @Delete(':id')
    @UseGuards(DIDAuthGuard)
    @ApiBearerAuth()
    @HttpCode(HttpStatus.OK)
    @ApiOperation({ summary: 'Revoke a credential' })
    @ApiResponse({ status: HttpStatus.OK, description: 'Credential revoked successfully' })
    @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'Credential not found' })
    @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
    async revokeCredential(
      @Param('id') id: string,
      @Req() req,
    ): Promise<{ success: boolean }> {
      const credential = await this.credentialsService.getCredentialById(id);

      if (!credential) {
        throw new NotFoundException('Credential not found');
      }

      // Check if the authenticated user owns this credential
      if (credential.did !== req.user.did) {
        throw new ForbiddenException('You do not have permission to revoke this credential');
      }

      const success = await this.credentialsService.revokeCredential(id, req);
      return { success };
    }
  
    @Get('did/:did')
    @UseGuards(DIDAuthGuard, DIDOwnerGuard)
    @ApiBearerAuth()
    @ApiOperation({ summary: 'Get all credentials for a DID' })
    @ApiResponse({ status: HttpStatus.OK, description: 'List of credentials' })
    @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
    async getCredentialsByDid(@Param('did') did: string) {
      return this.credentialsService.getCredentialsByDid(did);
    }

    @Get('my')
    @UseGuards(DIDAuthGuard)
    @ApiBearerAuth()
    @ApiOperation({ summary: 'Get all credentials for the authenticated user' })
    @ApiResponse({ status: HttpStatus.OK, description: 'List of credentials' })
    @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
    async getMyCredentials(@Req() req) {
      return this.credentialsService.getCredentialsByDid(req.user.did);
    }
  }