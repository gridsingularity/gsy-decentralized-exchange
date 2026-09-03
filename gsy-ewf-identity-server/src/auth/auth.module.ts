import { Module } from '@nestjs/common';
import { JwtModule } from '@nestjs/jwt';
import { PassportModule } from '@nestjs/passport';
import { ConfigModule, ConfigService } from '@nestjs/config';
import { MongooseModule } from '@nestjs/mongoose';
import { AuthController } from './auth.controller';
import { AuthService } from './auth.service';
import { DIDStrategy } from './strategies/did.strategy';
import { DIDModule } from '../did/did.module';
import { AuditModule } from '../audit/audit.module';
import { Challenge, ChallengeSchema } from '../database/schemas/challenge.schema';
import { User, UserSchema } from '../database/schemas/user.schema';

@Module({
  imports: [
    PassportModule,
    JwtModule.registerAsync({
      imports: [ConfigModule],
      useFactory: async (configService: ConfigService) => ({
        secret: configService.get<string>('jwt.secret'),
        signOptions: { expiresIn: '24h' },
      }),
      inject: [ConfigService],
    }),
    MongooseModule.forFeature([
      { name: Challenge.name, schema: ChallengeSchema },
      { name: User.name, schema: UserSchema },
    ]),
    DIDModule,
    AuditModule,
  ],
  controllers: [AuthController],
  providers: [AuthService, DIDStrategy],
  exports: [AuthService],
})
export class AuthModule {}