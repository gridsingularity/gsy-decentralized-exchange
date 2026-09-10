import { Module } from '@nestjs/common';
import { AssetKeyService } from './asset-key.service';
import { AssetDIDService } from './asset-did.service';
import { AssetDIDController } from './asset-did.controller';
import { DatabaseModule } from '../database/database.module';
import { AuditModule } from '../audit/audit.module';
import { CredentialsModule } from '../credentials/credentials.module';

/**
 * Asset / community DID module: key derivation, persistence and the machine-facing API.
 *
 * `DatabaseModule` supplies the `AssetDID` model; `AuditModule` supplies `AuditService`.
 * `ConfigModule` is global (`app.module.ts:14-17`), so `ConfigService` resolves without
 * importing it here.
 *
 * `DIDModule` is deliberately not imported: phase 1 writes nothing on-chain and resolution
 * goes through the existing `GET /did/:did`, so nothing here needs `DIDService` yet.
 *
 * `CredentialsModule` is imported for `POST /asset-dids/:subjectUuid/credential` (phase 4).
 * The dependency runs one way only - `CredentialsModule` knows nothing about assets - so
 * there is no cycle to break with `forwardRef`, and it should stay that way.
 *
 * NOTE: importing this module into `AppModule` means the server will not boot without a
 * valid `ASSET_DID_MASTER_SEED`, nor without an `API_KEY` (the class-level `ApiKeyGuard`
 * on `AssetDIDController` throws at construction when none is configured).
 */
@Module({
  imports: [DatabaseModule, AuditModule, CredentialsModule],
  controllers: [AssetDIDController],
  providers: [AssetKeyService, AssetDIDService],
  exports: [AssetKeyService, AssetDIDService],
})
export class AssetsModule {}
