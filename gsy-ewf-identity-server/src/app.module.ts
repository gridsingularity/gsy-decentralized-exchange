import { Module } from '@nestjs/common';
import { ConfigModule, ConfigService } from '@nestjs/config';
import { MongooseModule } from '@nestjs/mongoose';
import configuration from './config/configuration';
import { DIDModule } from './did/did.module';
import { DatabaseModule } from './database/database.module';
import { AuditModule } from './audit/audit.module';
import { AuthModule } from './auth/auth.module';
import { CredentialsModule } from './credentials/credentials.module';

@Module({
  imports: [
    // Configuration Module
    ConfigModule.forRoot({
      isGlobal: true,
      load: [configuration],
    }),
    
    // Database Module
    MongooseModule.forRootAsync({
      imports: [ConfigModule],
      useFactory: async (configService: ConfigService) => ({
        uri: configService.get<string>('mongodb.uri'),
        useNewUrlParser: true,
        useUnifiedTopology: true,
      }),
      inject: [ConfigService],
    }),
    
    // Application Modules
    DatabaseModule,
    DIDModule,
    AuditModule,
    AuthModule,
    CredentialsModule,
  ],
})
export class AppModule {}