import { Module } from '@nestjs/common';
import { MongooseModule } from '@nestjs/mongoose';
import { 
  User, UserSchema,
  AuditLog, AuditLogSchema,
  Challenge, ChallengeSchema,
  Credential, CredentialSchema,
  AssetDID, AssetDIDSchema
} from './schemas';

@Module({
  imports: [
    MongooseModule.forFeature([
      { name: AuditLog.name, schema: AuditLogSchema },
      { name: User.name, schema: UserSchema },
      { name: Challenge.name, schema: ChallengeSchema },
      { name: Credential.name, schema: CredentialSchema },
      { name: AssetDID.name, schema: AssetDIDSchema },
    ]),
  ],
  exports: [MongooseModule],
})
export class DatabaseModule {}