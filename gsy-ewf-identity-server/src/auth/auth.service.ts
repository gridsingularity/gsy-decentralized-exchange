import { Injectable, Logger, BadRequestException, UnauthorizedException } from '@nestjs/common';
import { JwtService } from '@nestjs/jwt';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { v4 as uuidv4 } from 'uuid';
import * as ethers from 'ethers';
import { Challenge } from '../database/schemas/challenge.schema';
import { User } from '../database/schemas/user.schema';
import { DIDService } from '../did/did.service';
import { AuditService } from '../audit/audit.service';
import { AuditAction } from '../database/schemas';
import { AuthChallengeResponse } from './dto/auth-challenge.dto';
import { AuthVerificationResponse } from './dto/auth-verification.dto';
import { UserInfoDto } from './dto/user-info.dto'

@Injectable()
export class AuthService {
  private readonly logger = new Logger(AuthService.name);

  constructor(
    @InjectModel(Challenge.name) private challengeModel: Model<Challenge>,
    @InjectModel(User.name) private userModel: Model<User>,
    private jwtService: JwtService,
    private didService: DIDService,
    private auditService: AuditService,
  ) {}

  async generateChallenge(did: string): Promise<AuthChallengeResponse> {
    const isRegistered = await this.didService.isDIDRegistered(did);
    if (!isRegistered) {
      throw new BadRequestException('DID is not registered');
    }

    const challengeId = uuidv4();
    const timestamp = new Date();
    const message = `Sign this message to authenticate with GSY EWF Identity Server: ${challengeId} at ${timestamp.toISOString()}`;

    const challenge = new this.challengeModel({
      id: challengeId,
      challenge: message,
      did,
      timestamp,
    });
    await challenge.save();

    await this.auditService.log(
      AuditAction.LOGIN_ATTEMPT,
      did,
      null,
      { challengeId },
    );

    return {
      id: challengeId,
      challenge: message,
      timestamp,
    };
  }

  async verifyChallenge(did: string, challengeId: string, signature: string): Promise<AuthVerificationResponse> {
    const challenge = await this.challengeModel.findOne({
      id: challengeId,
      did,
      used: false,
    }).exec();

    if (!challenge) {
      throw new BadRequestException('Invalid or expired challenge');
    }

    try {
      const recoveredAddress = ethers.verifyMessage(challenge.challenge, signature);
      
      const didAddress = did.split(':')[2];
      
      if (recoveredAddress.toLowerCase() !== didAddress.toLowerCase()) {
        throw new UnauthorizedException('Invalid signature');
      }

      challenge.used = true;
      await challenge.save();

      let user = await this.userModel.findOne({ did }).exec();
      if (!user) {
        user = new this.userModel({ did });
        await user.save();
      }

      const payload = { sub: did };
      const accessToken = this.jwtService.sign(payload);

      await this.auditService.log(
        AuditAction.LOGIN_SUCCESS,
        did,
        null,
        { challengeId },
      );

      return {
        accessToken,
        did,
      };
    } catch (error) {
      await this.auditService.log(
        AuditAction.LOGIN_FAILURE,
        did,
        null,
        { challengeId, error: error.message },
        null,
        false,
      );

      if (error instanceof UnauthorizedException) {
        throw error;
      }
      this.logger.error(`Error verifying challenge: ${error.message}`);
      throw new UnauthorizedException('Invalid signature');
    }
  }

  async validateUser(did: string): Promise<UserInfoDto | null> {
    const user = await this.userModel.findOne({ did }).lean().exec();
    if (!user) {
      return null;
    }
    return {
      did: user.did,
      gsyDexAddress: user.gsyDexAddress,
      hasVerifiedCredential: user.hasVerifiedCredential,
      metadata: user.metadata,
    }
  }
}