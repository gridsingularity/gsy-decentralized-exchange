import { Test, TestingModule } from '@nestjs/testing';
import { CredentialsController } from '../src/credentials/credentials.controller';
import { CredentialsService } from '../src/credentials/credentials.service';
import { DIDAuthGuard } from '../src/auth/guards/did-auth.guard';
import { DIDOwnerGuard } from '../src/auth/guards/did-owner.guard';
import { ApiKeyOrJwtGuard } from '../src/auth/guards/api-key-or-jwt.guard';
import { ForbiddenException, NotFoundException } from '@nestjs/common';

// Mock the credentials service
const mockCredentialsService = {
  getCredentialsByDid: jest.fn(),
  getCredentialById: jest.fn(),
  revokeCredential: jest.fn(),
  issueCredential: jest.fn(),
  verifyCredential: jest.fn(),
};

// Mock the DIDAuthGuard
const mockDIDAuthGuard = { canActivate: jest.fn(() => true) };

// Mock the DIDOwnerGuard
const mockDIDOwnerGuard = { canActivate: jest.fn(() => true) };

// Mock the composite guard on DELETE /credentials/:id. Overridden here because the real
// one needs a ConfigService with an API key; its own behaviour is covered by
// test/api-key-or-jwt.guard.spec.ts.
const mockApiKeyOrJwtGuard = { canActivate: jest.fn(() => true) };

describe('CredentialsController', () => {
  let controller: CredentialsController;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [CredentialsController],
      providers: [
        { provide: CredentialsService, useValue: mockCredentialsService },
      ],
    })
      .overrideGuard(DIDAuthGuard)
      .useValue(mockDIDAuthGuard)
      .overrideGuard(DIDOwnerGuard)
      .useValue(mockDIDOwnerGuard)
      .overrideGuard(ApiKeyOrJwtGuard)
      .useValue(mockApiKeyOrJwtGuard)
      .compile();

    controller = module.get<CredentialsController>(CredentialsController);

    // Reset mock function calls before each test
    jest.clearAllMocks();
  });

  it('should be defined', () => {
    expect(controller).toBeDefined();
  });

  describe('getCredentialsByDid', () => {
    it('should return credentials for a DID when user is authorized', async () => {
      const mockCredentials = [{ id: 'credential1', did: 'did:ethr:0x123' }];
      mockCredentialsService.getCredentialsByDid.mockResolvedValue(mockCredentials);
      mockDIDOwnerGuard.canActivate.mockReturnValue(true);

      const result = await controller.getCredentialsByDid('did:ethr:0x123');
      expect(result).toEqual(mockCredentials);
      expect(mockCredentialsService.getCredentialsByDid).toHaveBeenCalledWith('did:ethr:0x123');
    });

    // The actual guard denial is tested in the guard's unit tests
    // Here we just verify the controller calls the service with correct params
  });

  describe('revokeCredential', () => {
    it('should revoke a credential when user is the owner', async () => {
      const credentialId = 'credential1';
      const mockCredential = { id: credentialId, did: 'did:ethr:0x123' };
      const mockReq = { user: { did: 'did:ethr:0x123' } };
      
      mockCredentialsService.getCredentialById.mockResolvedValue(mockCredential);
      mockCredentialsService.revokeCredential.mockResolvedValue(true);

      const result = await controller.revokeCredential(credentialId, mockReq);
      expect(result).toEqual({ success: true });
      expect(mockCredentialsService.getCredentialById).toHaveBeenCalledWith(credentialId);
      expect(mockCredentialsService.revokeCredential).toHaveBeenCalledWith(credentialId, mockReq);
    });

    it('should throw ForbiddenException when user is not the credential owner', async () => {
      const credentialId = 'credential1';
      const mockCredential = { id: credentialId, did: 'did:ethr:0x456' }; // Different DID
      const mockReq = { user: { did: 'did:ethr:0x123' } };
      
      mockCredentialsService.getCredentialById.mockResolvedValue(mockCredential);

      await expect(controller.revokeCredential(credentialId, mockReq))
        .rejects.toThrow(ForbiddenException);
      expect(mockCredentialsService.getCredentialById).toHaveBeenCalledWith(credentialId);
      expect(mockCredentialsService.revokeCredential).not.toHaveBeenCalled();
    });

    /**
     * The machine path (plan §2.7). Before this, `DELETE /credentials/:id` was
     * `@UseGuards(DIDAuthGuard)` and compared `credential.did !== req.user.did` - a check
     * NO machine caller can satisfy, which would have left every asset credential
     * unrevocable through the API.
     */
    it('should revoke any credential for an x-api-key machine caller', async () => {
      const credentialId = 'asset-credential';
      // An asset DID: no principal can ever authenticate as this, by construction.
      const mockCredential = { id: credentialId, did: 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21' };
      const mockReq = { authKind: 'api-key' }; // stamped by ApiKeyOrJwtGuard; no req.user

      mockCredentialsService.getCredentialById.mockResolvedValue(mockCredential);
      mockCredentialsService.revokeCredential.mockResolvedValue(true);

      const result = await controller.revokeCredential(credentialId, mockReq);

      expect(result).toEqual({ success: true });
      expect(mockCredentialsService.revokeCredential).toHaveBeenCalledWith(credentialId, mockReq);
    });

    it('should still 404 for a machine caller when the credential does not exist', async () => {
      mockCredentialsService.getCredentialById.mockResolvedValue(null);

      await expect(controller.revokeCredential('nope', { authKind: 'api-key' }))
        .rejects.toThrow(NotFoundException);
      expect(mockCredentialsService.revokeCredential).not.toHaveBeenCalled();
    });

    it('should fail closed when the request carries neither authKind nor a user', async () => {
      // Unreachable through HTTP - ApiKeyOrJwtGuard 401s first - but asserted because the
      // handler must not treat "no authKind" as "machine caller". The ownership branch is
      // the default, and it denies.
      const mockCredential = { id: 'credential1', did: 'did:ethr:0x123' };
      mockCredentialsService.getCredentialById.mockResolvedValue(mockCredential);

      await expect(controller.revokeCredential('credential1', {}))
        .rejects.toThrow(ForbiddenException);
      expect(mockCredentialsService.revokeCredential).not.toHaveBeenCalled();
    });

    it('is guarded by ApiKeyOrJwtGuard, and no longer by DIDAuthGuard alone', () => {
      const guards = Reflect.getMetadata('__guards__', CredentialsController.prototype.revokeCredential);

      expect(guards).toEqual([ApiKeyOrJwtGuard]);
    });

    it('should throw NotFoundException when credential does not exist', async () => {
      const credentialId = 'nonexistent';
      const mockReq = { user: { did: 'did:ethr:0x123' } };
      
      mockCredentialsService.getCredentialById.mockResolvedValue(null);

      await expect(controller.revokeCredential(credentialId, mockReq))
        .rejects.toThrow(NotFoundException);
      expect(mockCredentialsService.getCredentialById).toHaveBeenCalledWith(credentialId);
      expect(mockCredentialsService.revokeCredential).not.toHaveBeenCalled();
    });
  });

  describe('getMyCredentials', () => {
    it('should return credentials for the authenticated user', async () => {
      const mockCredentials = [{ id: 'credential1', did: 'did:ethr:0x123' }];
      const mockReq = { user: { did: 'did:ethr:0x123' } };
      
      mockCredentialsService.getCredentialsByDid.mockResolvedValue(mockCredentials);

      const result = await controller.getMyCredentials(mockReq);
      expect(result).toEqual(mockCredentials);
      expect(mockCredentialsService.getCredentialsByDid).toHaveBeenCalledWith('did:ethr:0x123');
    });
  });
});