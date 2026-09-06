import { Module } from '@nestjs/common';
import { AssetKeyService } from './asset-key.service';

/**
 * Asset / community DID module.
 *
 * Minimal for now: key derivation only. The `AssetDID` schema, `AssetDIDService` and
 * `AssetDIDController` arrive with unit 3.
 *
 * `ConfigModule` is global, so `ConfigService` resolves without importing it here.
 *
 * NOTE: importing this module into `AppModule` means the server will not boot without a
 * valid `ASSET_DID_MASTER_SEED`.
 */
@Module({
  providers: [AssetKeyService],
  exports: [AssetKeyService],
})
export class AssetsModule {}
