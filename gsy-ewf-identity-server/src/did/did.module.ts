import { Module } from '@nestjs/common';
import { ConfigModule } from '@nestjs/config';
import { DIDController } from './did.controller';
import { DIDService } from './did.service';
import { DatabaseModule } from '../database/database.module';
import { AuditModule } from '../audit/audit.module';

@Module({
  imports: [
    ConfigModule,
    DatabaseModule,
    AuditModule,
  ],
  controllers: [DIDController],
  providers: [DIDService],
  exports: [DIDService],
})
export class DIDModule {}