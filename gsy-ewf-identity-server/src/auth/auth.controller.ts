import { Controller, Post, Body, HttpCode, HttpStatus } from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse } from '@nestjs/swagger';
import { AuthService } from './auth.service';
import { AuthChallengeRequest, AuthChallengeResponse } from './dto/auth-challenge.dto';
import { AuthVerificationRequest, AuthVerificationResponse } from './dto/auth-verification.dto';

@ApiTags('Authentication')
@Controller('auth')
export class AuthController {
  constructor(private readonly authService: AuthService) {}

  @Post('challenge')
  @HttpCode(HttpStatus.OK)
  @ApiOperation({ summary: 'Generate authentication challenge' })
  @ApiResponse({ status: HttpStatus.OK, description: 'Challenge generated', type: AuthChallengeResponse })
  @ApiResponse({ status: HttpStatus.BAD_REQUEST, description: 'Invalid input' })
  async generateChallenge(@Body() request: AuthChallengeRequest): Promise<AuthChallengeResponse> {
    return this.authService.generateChallenge(request.did);
  }

  @Post('verify')
  @HttpCode(HttpStatus.OK)
  @ApiOperation({ summary: 'Verify authentication challenge' })
  @ApiResponse({ status: HttpStatus.OK, description: 'Authentication successful', type: AuthVerificationResponse })
  @ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'Invalid signature' })
  async verifyChallenge(@Body() request: AuthVerificationRequest): Promise<AuthVerificationResponse> {
    return this.authService.verifyChallenge(request.did, request.challengeId, request.signature);
  }
}