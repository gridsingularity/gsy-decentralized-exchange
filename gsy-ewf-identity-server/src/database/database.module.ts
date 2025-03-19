import { Module } from '@nestjs/common';
import { MongooseModule } from '@nestjs/mongoose';
import { 
  User, UserSchema,
  AuditLog, AuditLogSchema,
  Challenge, ChallengeSchema,
  Credential, CredentialSchema
} from './schemas';

@Module({
  imports: [
    MongooseModule.forFeature([
      { name: AuditLog.name, schema: AuditLogSchema },
      { name: User.name, schema: UserSchema },
      { name: Challenge.name, schema: ChallengeSchema },
      { name: Credential.name, schema: CredentialSchema },
    ]),
  ],
  exports: [MongooseModule],
})
export class DatabaseModule {}