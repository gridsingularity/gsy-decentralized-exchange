import { 
    Controller, Post, Get, Delete, Body, Param, 
    UseGuards, Req, HttpCode, HttpStatus, ForbiddenException,
    NotFoundException
  } from '@nestjs/common';
  import { ApiTags, ApiOperation, ApiResponse, ApiBearerAuth, ApiHeader } from '@nestjs/swagger';
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
  import { ApiKeyOrJwtGuard, MACHINE_AUTH_KIND } from '../auth/guards/api-key-or-jwt.guard';

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
    @UseGuards(ApiKeyOrJwtGuard)
    @ApiBearerAuth()
    @ApiHeader({
      name: 'x-api-key',
      required: false,
      description:
        'Machine-to-machine API key. Send this OR a bearer token. A machine caller may ' +
        'revoke any credential, including asset credentials, which no JWT principal can ' +
        'own; a JWT caller may revoke only its own.',
    })
    @HttpCode(HttpStatus.OK)
    @ApiOperation({ summary: 'Revoke a credential' })
    @ApiResponse({ status: HttpStatus.OK, description: 'Credential revoked successfully' })
    @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'Credential not found' })
    @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Unauthorized' })
    @ApiResponse({ status: HttpStatus.FORBIDDEN, description: 'JWT caller does not own the credential' })
    async revokeCredential(
      @Param('id') id: string,
      @Req() req,
    ): Promise<{ success: boolean }> {
      const credential = await this.credentialsService.getCredentialById(id);

      if (!credential) {
        throw new NotFoundException('Credential not found');
      }

      // Ownership applies to the JWT path only. A machine caller holds the shared API key
      // and has no DID to compare against - and an asset credential's `did` is the ASSET's,
      // which no principal can ever authenticate as, so requiring a match would make asset
      // credentials unrevocable (plan §2.7).
      //
      // The polarity is deliberate: anything that is not positively identified as the
      // machine path takes the ownership check, so a missing `authKind` (a misconfigured or
      // stubbed guard) fails closed on `req.user` being undefined rather than skipping the
      // check.
      if (req.authKind !== MACHINE_AUTH_KIND) {
        if (credential.did !== req.user?.did) {
          throw new ForbiddenException('You do not have permission to revoke this credential');
        }
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