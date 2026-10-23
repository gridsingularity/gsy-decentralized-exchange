import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { ExecutionContext, UnauthorizedException } from '@nestjs/common';
import { ApiKeyGuard } from '../src/auth/guards/api-key.guard';

const VALID_KEY = 'fedecom_user';

function mockContext(headers: Record<string, any>): ExecutionContext {
  return {
    switchToHttp: jest.fn().mockReturnValue({
      getRequest: jest.fn().mockReturnValue({ headers }),
    }),
  } as unknown as ExecutionContext;
}

function configWith(apiKey: any): ConfigService {
  return {
    get: jest.fn((key: string) => (key === 'identity.apiKey' ? apiKey : undefined)),
  } as unknown as ConfigService;
}

describe('ApiKeyGuard', () => {
  let guard: ApiKeyGuard;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        ApiKeyGuard,
        { provide: ConfigService, useValue: configWith(VALID_KEY) },
      ],
    }).compile();

    guard = module.get<ApiKeyGuard>(ApiKeyGuard);
  });

  it('should be defined', () => {
    expect(guard).toBeDefined();
  });

  it('should allow access when the x-api-key header matches the configured key', () => {
    const context = mockContext({ 'x-api-key': VALID_KEY });
    expect(guard.canActivate(context)).toBe(true);
  });

  it('should deny access when the x-api-key header is missing', () => {
    const context = mockContext({});
    expect(() => guard.canActivate(context)).toThrow(UnauthorizedException);
    expect(() => guard.canActivate(context)).toThrow('invalid or missing x-api-key');
  });

  it('should deny access when there are no headers at all', () => {
    const context = {
      switchToHttp: jest.fn().mockReturnValue({
        getRequest: jest.fn().mockReturnValue({}),
      }),
    } as unknown as ExecutionContext;

    expect(() => guard.canActivate(context)).toThrow(UnauthorizedException);
  });

  it('should deny access when the x-api-key header is empty', () => {
    const context = mockContext({ 'x-api-key': '' });
    expect(() => guard.canActivate(context)).toThrow(UnauthorizedException);
  });

  it('should deny access for a wrong key of the same length', () => {
    // Same length as VALID_KEY, so this exercises the digest comparison itself
    // rather than the length-mismatch path below.
    const sameLengthWrongKey = 'x'.repeat(VALID_KEY.length);
    const context = mockContext({ 'x-api-key': sameLengthWrongKey });

    expect(sameLengthWrongKey).toHaveLength(VALID_KEY.length);
    expect(() => guard.canActivate(context)).toThrow(UnauthorizedException);
    expect(() => guard.canActivate(context)).toThrow('invalid or missing x-api-key');
  });

  it('should deny a wrong-length key with 401 and never throw out of timingSafeEqual', () => {
    // crypto.timingSafeEqual throws a RangeError/TypeError on buffers of unequal
    // length, which would surface as a 500. The guard hashes both sides to a fixed
    // 32-byte digest first, so these must all be plain UnauthorizedExceptions.
    const wrongLengthKeys = ['a', VALID_KEY + 'extra', 'z'.repeat(4096)];

    for (const key of wrongLengthKeys) {
      const context = mockContext({ 'x-api-key': key });
      let thrown: unknown;
      try {
        guard.canActivate(context);
      } catch (error) {
        thrown = error;
      }

      expect(thrown).toBeInstanceOf(UnauthorizedException);
      expect((thrown as UnauthorizedException).message).toBe('invalid or missing x-api-key');
    }
  });

  it('should deny access when the header is not a string (repeated header)', () => {
    const context = mockContext({ 'x-api-key': [VALID_KEY, VALID_KEY] });
    expect(() => guard.canActivate(context)).toThrow(UnauthorizedException);
  });

  describe('fail-closed construction', () => {
    // This DELIBERATELY INVERTS gsy-offchain-storage's behaviour, where an empty
    // configured key means allow-all (gsy-offchain-storage/src/startup.rs:24, and
    // gsy-offchain-storage/tests/api/auth.rs:49-52). See plan §2.6 deviation 3.
    // These assertions exist so that a future "parity with the Rust service" refactor
    // cannot silently reopen POST /did and credential issuance to unauthenticated
    // callers. If you are here to make an empty key mean allow-all: don't.
    it.each([
      ['an empty string', ''],
      ['whitespace only', '   '],
      ['undefined (unset env var)', undefined],
      ['null', null],
    ])('should throw at construction when the configured key is %s', (_label, value) => {
      expect(() => new ApiKeyGuard(configWith(value))).toThrow();
    });

    it('should not construct through the Nest DI container with an unset key', async () => {
      await expect(
        Test.createTestingModule({
          providers: [
            ApiKeyGuard,
            { provide: ConfigService, useValue: configWith('') },
          ],
        })
          .compile()
          .then((module) => module.get<ApiKeyGuard>(ApiKeyGuard)),
      ).rejects.toThrow();
    });
  });
});
