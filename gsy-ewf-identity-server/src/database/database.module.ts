import { Module } from '@nestjs/common';
import { MongooseModule } from '@nestjs/mongoose';
import { 
  User, UserSchema,
  AuditLog, AuditLogSchema
} from './schemas';

@Module({
  imports: [
    MongooseModule.forFeature([
      { name: AuditLog.name, schema: AuditLogSchema },
      { name: User.name, schema: UserSchema },
    ]),
  ],
  exports: [MongooseModule],
})
export class DatabaseModule {}