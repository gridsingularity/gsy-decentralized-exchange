import {
  Injectable,
  CanActivate,
  ExecutionContext,
  UnauthorizedException,
} from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { createHash, timingSafeEqual } from 'crypto';

/**
 * Machine-to-machine auth for endpoints that no human principal can reach.
 *
 * Contract mirrors `gsy-offchain-storage/src/startup.rs:23-39`: the caller must send
 * an `x-api-key` header equal to the configured key, otherwise 401 with the exact
 * message `invalid or missing x-api-key` (kept identical for operator parity).
 *
 * Three deliberate deviations from the Rust implementation:
 *  1. Constant-time comparison (`timingSafeEqual` over SHA-256 digests) instead of `==`.
 *  2. No `/health_check` path exemption - this guard is attached per-route, not globally.
 *  3. FAIL CLOSED. `gsy-offchain-storage` treats an empty configured key as allow-all
 *     (`startup.rs:24`). This service holds the issuer key (and, later, the asset-DID
 *     master seed), so an unset key must prevent the service from starting rather than
 *     serve the guarded routes open. Nest instantiates guards at bootstrap, so throwing
 *     from the constructor is what makes that a boot failure.
 */
@Injectable()
export class ApiKeyGuard implements CanActivate {
  private readonly expectedDigest: Buffer;

  constructor(private readonly configService: ConfigService) {
    const configured = this.configService.get<string>('identity.apiKey');

    if (typeof configured !== 'string' || configured.trim().length === 0) {
      throw new Error(
        'ApiKeyGuard: no API key configured. Set IDENTITY_API_KEY (or API_KEY) to a ' +
          'non-empty value. This service refuses to start with an unset key rather ' +
          'than serving guarded routes open.',
      );
    }

    this.expectedDigest = ApiKeyGuard.digest(configured);
  }

  canActivate(context: ExecutionContext): boolean {
    const request = context.switchToHttp().getRequest();
    // Node lowercases incoming header names.
    const provided = request?.headers?.['x-api-key'];

    if (typeof provided !== 'string' || provided.length === 0) {
      throw new UnauthorizedException('invalid or missing x-api-key');
    }

    // `timingSafeEqual` throws on a length mismatch, so never feed it the raw keys:
    // hashing first yields two fixed-length (32 byte) buffers, so a wrong-length key
    // is a plain 401 rather than a 500.
    if (!timingSafeEqual(ApiKeyGuard.digest(provided), this.expectedDigest)) {
      throw new UnauthorizedException('invalid or missing x-api-key');
    }

    return true;
  }

  private static digest(value: string): Buffer {
    return createHash('sha256').update(value, 'utf8').digest();
  }
}
