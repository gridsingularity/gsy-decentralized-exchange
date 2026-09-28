import { Module } from '@nestjs/common';
import { MongooseModule } from '@nestjs/mongoose';
import { ConfigModule } from '@nestjs/config';
import { CredentialsController } from './credentials.controller';
import { CredentialsService } from './credentials.service';
import { DIDModule } from '../did/did.module';
import { AuditModule } from '../audit/audit.module';
import { Credential, CredentialSchema } from '../database/schemas/credential.schema';
import { User, UserSchema } from '../database/schemas/user.schema';
import { AuthModule } from '../auth/auth.module';
import { ApiKeyGuard } from '../auth/guards/api-key.guard';
import { DIDAuthGuard } from '../auth/guards/did-auth.guard';
import { ApiKeyOrJwtGuard } from '../auth/guards/api-key-or-jwt.guard';

@Module({
  imports: [
    ConfigModule,
    MongooseModule.forFeature([
      { name: Credential.name, schema: CredentialSchema },
      { name: User.name, schema: UserSchema },
    ]),
    DIDModule,
    AuditModule,
    AuthModule,
  ],
  controllers: [CredentialsController],
  // The two delegates of `ApiKeyOrJwtGuard` are providers rather than `new`-ed inside it,
  // so each can be substituted independently in a test and so `ApiKeyGuard` keeps its
  // fail-closed constructor behaviour (no API key configured => the app does not boot).
  providers: [CredentialsService, ApiKeyGuard, DIDAuthGuard, ApiKeyOrJwtGuard],
  exports: [CredentialsService],
})
export class CredentialsModule {}