import { CanActivate, ExecutionContext, Injectable, UnauthorizedException } from '@nestjs/common';
import { Observable, lastValueFrom, isObservable } from 'rxjs';
import { ApiKeyGuard } from './api-key.guard';
import { DIDAuthGuard } from './did-auth.guard';

/** How the request authenticated. Stamped on the request by `ApiKeyOrJwtGuard`. */
export type AuthKind = 'api-key' | 'jwt';

/** The machine path. Compared against explicitly rather than inferred from `req.user`. */
export const MACHINE_AUTH_KIND: AuthKind = 'api-key';

/**
 * Accept EITHER an `x-api-key` machine caller OR a JWT human caller, and record which.
 *
 * Why this exists (plan §2.7): `DELETE /credentials/:id` was `@UseGuards(DIDAuthGuard)` and
 * the handler compared `credential.did !== req.user.did`. That is correct for a human
 * revoking their own credential and IMPOSSIBLE for a machine: an asset has no Substrate
 * account, can never obtain a JWT, and its credential's `did` is the asset's, not any
 * caller's. Asset credentials would have been unrevocable through the API.
 *
 * WHY ONE COMPOSITE GUARD RATHER THAN TWO ROUTES. The alternative - a second
 * `DELETE /asset-dids/:subjectUuid/credential` under `ApiKeyGuard` - would mean two
 * revocation paths, two authorisation rules and two chances for one of them to drift from
 * `revokeCredential`'s semantics. Revocation is one operation; it should have one handler.
 * Nest's `@UseGuards(A, B)` is AND, not OR, so an OR needs an explicit composite.
 *
 * WHY THE HEADER SELECTS THE PATH, AND A BAD KEY DOES NOT FALL THROUGH. If `x-api-key` is
 * present, this is a machine caller and `ApiKeyGuard` decides, full stop. Trying the JWT
 * path after a failed key would turn "wrong key" into a second guess against a different
 * credential type, and would let a caller probe which of the two failed. A present-but-wrong
 * key is a 401 with the same message the off-chain storage uses.
 *
 * The guard sets `request.authKind`; the handler must branch on it explicitly rather than
 * sniffing `req.user`, so that a future guard change cannot silently skip an ownership
 * check. Nothing here decides ownership - that stays in the handler, which is the only
 * place that knows what the resource is.
 */
@Injectable()
export class ApiKeyOrJwtGuard implements CanActivate {
  constructor(
    private readonly apiKeyGuard: ApiKeyGuard,
    private readonly didAuthGuard: DIDAuthGuard,
  ) {}

  async canActivate(context: ExecutionContext): Promise<boolean> {
    const request = context.switchToHttp().getRequest();

    // Node lowercases incoming header names.
    const apiKeyHeader = request?.headers?.['x-api-key'];

    if (typeof apiKeyHeader === 'string' && apiKeyHeader.length > 0) {
      // Throws `UnauthorizedException('invalid or missing x-api-key')` on a wrong key.
      const allowed = await resolve(this.apiKeyGuard.canActivate(context));

      if (!allowed) {
        throw new UnauthorizedException('invalid or missing x-api-key');
      }

      request.authKind = MACHINE_AUTH_KIND;
      // No `request.user`: there is no principal here, only a shared secret. Leaving it
      // undefined is what makes an accidental ownership check fail closed.
      return true;
    }

    // Passport's guard throws UnauthorizedException itself when the bearer token is
    // missing or invalid, so an unauthenticated request 401s here.
    const allowed = await resolve(this.didAuthGuard.canActivate(context));

    if (!allowed) {
      throw new UnauthorizedException();
    }

    request.authKind = 'jwt';
    return true;
  }
}

/** `CanActivate` may return a boolean, a promise or an observable; normalise all three. */
async function resolve(
  result: boolean | Promise<boolean> | Observable<boolean>,
): Promise<boolean> {
  if (isObservable(result)) {
    return lastValueFrom(result);
  }
  return result;
}
