import { Module } from '@nestjs/common';
import { MongooseModule } from '@nestjs/mongoose';
import { 
  User, UserSchema,
  AuditLog, AuditLogSchema,
  Challenge, ChallengeSchema
} from './schemas';

@Module({
  imports: [
    MongooseModule.forFeature([
      { name: AuditLog.name, schema: AuditLogSchema },
      { name: User.name, schema: UserSchema },
      { name: Challenge.name, schema: ChallengeSchema },
    ]),
  ],
  exports: [MongooseModule],
})
export class DatabaseModule {}