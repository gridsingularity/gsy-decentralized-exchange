import { DIDOwnerGuard } from '../src/auth/guards/did-owner.guard';
import { ExecutionContext, ForbiddenException } from '@nestjs/common';

describe('DIDOwnerGuard', () => {
  let guard: DIDOwnerGuard;
  
  beforeEach(() => {
    guard = new DIDOwnerGuard();
  });

  it('should be defined', () => {
    expect(guard).toBeDefined();
  });

  it('should allow access when user DID matches requested DID', () => {
    // Create mock execution context
    const context = {
      switchToHttp: jest.fn().mockReturnValue({
        getRequest: jest.fn().mockReturnValue({
          user: { did: 'did:ethr:0x123' },
          params: { did: 'did:ethr:0x123' }
        })
      })
    } as unknown as ExecutionContext;

    expect(guard.canActivate(context)).toBe(true);
  });

  it('should deny access when user DID does not match requested DID', () => {
    // Create mock execution context
    const context = {
      switchToHttp: jest.fn().mockReturnValue({
        getRequest: jest.fn().mockReturnValue({
          user: { did: 'did:ethr:0x123' },
          params: { did: 'did:ethr:0x456' }
        })
      })
    } as unknown as ExecutionContext;

    expect(() => guard.canActivate(context)).toThrow(ForbiddenException);
  });

  it('should deny access when user is not authenticated', () => {
    // Create mock execution context
    const context = {
      switchToHttp: jest.fn().mockReturnValue({
        getRequest: jest.fn().mockReturnValue({
          user: null,
          params: { did: 'did:ethr:0x123' }
        })
      })
    } as unknown as ExecutionContext;

    expect(guard.canActivate(context)).toBe(false);
  });

  it('should allow access when no DID is specified in the route', () => {
    // Create mock execution context
    const context = {
      switchToHttp: jest.fn().mockReturnValue({
        getRequest: jest.fn().mockReturnValue({
          user: { did: 'did:ethr:0x123' },
          params: {}
        })
      })
    } as unknown as ExecutionContext;

    expect(guard.canActivate(context)).toBe(true);
  });
});