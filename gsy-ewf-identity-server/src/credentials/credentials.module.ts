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
  providers: [CredentialsService],
  exports: [CredentialsService],
})
export class CredentialsModule {}