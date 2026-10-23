import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { ExecutionContext, UnauthorizedException } from '@nestjs/common';
import { of, throwError } from 'rxjs';
import { ApiKeyGuard } from '../src/auth/guards/api-key.guard';
import { DIDAuthGuard } from '../src/auth/guards/did-auth.guard';
import { ApiKeyOrJwtGuard } from '../src/auth/guards/api-key-or-jwt.guard';

const VALID_KEY = 'fedecom_user';

/** Returns both the context and the request object, so `authKind` can be inspected. */
function mockContext(headers: Record<string, any>) {
  const request: Record<string, any> = { headers };

  const context = {
    switchToHttp: jest.fn().mockReturnValue({
      getRequest: jest.fn().mockReturnValue(request),
    }),
  } as unknown as ExecutionContext;

  return { context, request };
}

describe('ApiKeyOrJwtGuard', () => {
  let guard: ApiKeyOrJwtGuard;
  let didAuthGuard: { canActivate: jest.Mock };

  beforeEach(async () => {
    // The real ApiKeyGuard, because its behaviour (constant-time compare, exact 401
    // message) is part of the contract being composed. Only passport is stubbed - it needs
    // a live strategy and an HTTP request otherwise.
    didAuthGuard = {
      canActivate: jest.fn(() => {
        throw new UnauthorizedException();
      }),
    };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        ApiKeyOrJwtGuard,
        ApiKeyGuard,
        { provide: DIDAuthGuard, useValue: didAuthGuard },
        {
          provide: ConfigService,
          useValue: {
            get: jest.fn((key: string) => (key === 'identity.apiKey' ? VALID_KEY : undefined)),
          },
        },
      ],
    }).compile();

    guard = module.get<ApiKeyOrJwtGuard>(ApiKeyOrJwtGuard);
  });

  it('should be defined', () => {
    expect(guard).toBeDefined();
  });

  describe('machine caller (x-api-key)', () => {
    it('allows a correct key and stamps authKind=api-key', async () => {
      const { context, request } = mockContext({ 'x-api-key': VALID_KEY });

      await expect(guard.canActivate(context)).resolves.toBe(true);
      expect(request.authKind).toBe('api-key');
      // No principal: the handler's ownership check must not be reachable via req.user.
      expect(request.user).toBeUndefined();
      expect(didAuthGuard.canActivate).not.toHaveBeenCalled();
    });

    it('rejects a wrong key WITHOUT falling through to the JWT path', async () => {
      // The whole point of selecting on header presence: a bad key is a 401, not a second
      // guess against a different credential type.
      const { context, request } = mockContext({
        'x-api-key': 'wrong',
        authorization: 'Bearer whatever',
      });

      await expect(guard.canActivate(context)).rejects.toThrow(UnauthorizedException);
      await expect(guard.canActivate(context)).rejects.toThrow('invalid or missing x-api-key');
      expect(didAuthGuard.canActivate).not.toHaveBeenCalled();
      expect(request.authKind).toBeUndefined();
    });

    it('rejects a wrong-length key without a 500 out of timingSafeEqual', async () => {
      const { context } = mockContext({ 'x-api-key': 'x' });

      await expect(guard.canActivate(context)).rejects.toThrow(UnauthorizedException);
    });

    it('falls through to the JWT path when the header is present but empty', async () => {
      // An empty header is not a machine caller; treat it as absent rather than as a
      // failed key, so a proxy that always sets the header does not lock humans out.
      didAuthGuard.canActivate.mockReturnValue(true);
      const { context, request } = mockContext({ 'x-api-key': '' });

      await expect(guard.canActivate(context)).resolves.toBe(true);
      expect(request.authKind).toBe('jwt');
    });
  });

  describe('JWT caller', () => {
    it('allows a valid token and stamps authKind=jwt', async () => {
      didAuthGuard.canActivate.mockReturnValue(true);
      const { context, request } = mockContext({ authorization: 'Bearer good' });

      await expect(guard.canActivate(context)).resolves.toBe(true);
      expect(request.authKind).toBe('jwt');
    });

    it('awaits a promise-returning passport guard', async () => {
      didAuthGuard.canActivate.mockReturnValue(Promise.resolve(true));
      const { context, request } = mockContext({ authorization: 'Bearer good' });

      await expect(guard.canActivate(context)).resolves.toBe(true);
      expect(request.authKind).toBe('jwt');
    });

    it('resolves an observable-returning passport guard', async () => {
      didAuthGuard.canActivate.mockReturnValue(of(true));
      const { context, request } = mockContext({ authorization: 'Bearer good' });

      await expect(guard.canActivate(context)).resolves.toBe(true);
      expect(request.authKind).toBe('jwt');
    });
  });

  describe('unauthenticated caller', () => {
    it('rejects a request with neither an API key nor a token', async () => {
      const { context, request } = mockContext({});

      await expect(guard.canActivate(context)).rejects.toThrow(UnauthorizedException);
      expect(request.authKind).toBeUndefined();
    });

    it('rejects when passport returns false rather than throwing', async () => {
      didAuthGuard.canActivate.mockReturnValue(false);
      const { context, request } = mockContext({ authorization: 'Bearer bad' });

      await expect(guard.canActivate(context)).rejects.toThrow(UnauthorizedException);
      expect(request.authKind).toBeUndefined();
    });

    it('propagates a rejecting observable from passport', async () => {
      didAuthGuard.canActivate.mockReturnValue(throwError(() => new UnauthorizedException()));
      const { context } = mockContext({ authorization: 'Bearer bad' });

      await expect(guard.canActivate(context)).rejects.toThrow(UnauthorizedException);
    });
  });

  it('refuses to construct when no API key is configured (fail closed)', async () => {
    // Inherited from ApiKeyGuard, and asserted here too: the composite must not become a
    // way to reach a guarded route with an unset key.
    await expect(
      Test.createTestingModule({
        providers: [
          ApiKeyOrJwtGuard,
          ApiKeyGuard,
          { provide: DIDAuthGuard, useValue: didAuthGuard },
          { provide: ConfigService, useValue: { get: jest.fn(() => '') } },
        ],
      }).compile(),
    ).rejects.toThrow(/no API key configured/);
  });
});
