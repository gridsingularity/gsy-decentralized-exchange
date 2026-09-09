import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getModelToken } from '@nestjs/mongoose';
import { BadRequestException, UnauthorizedException } from '@nestjs/common';
import { CredentialsService } from '../src/credentials/credentials.service';
import { DIDService } from '../src/did/did.service';
import { AuditService } from '../src/audit/audit.service';
import { Credential, CredentialStatus } from '../src/database/schemas/credential.schema';
import { User } from '../src/database/schemas/user.schema';
import { AuditAction } from '../src/database/schemas';

// Mock the substrate verification utils
jest.mock('../src/credentials/utils/substrate-verification', () => ({
  verifySubstrateSignature: jest.fn().mockImplementation(async (message, signature, address) => {
    // Always return true for valid_substrate_signature in tests
    if (signature === 'valid_substrate_signature') {
      return true;
    }
    // Return false for all other signatures
    return false;
  }),
  formatSubstrateSigningMessage: jest.fn(message => `<Bytes>${message}</Bytes>`),
}));

/**
 * Obviously-fake but structurally valid secp256k1 key. Valid matters: the `realEthers`
 * cases below build a real `Wallet` from it.
 */
const ISSUER_PRIVATE_KEY =
  '0x1234567890123456789012345678901234567890123456789012345678901234';

/**
 * Switches the `ethers` mock between the canned behaviour the older cases depend on and
 * the real library.
 *
 * The canned behaviour is why bug (B) survived so long: with `verifyMessage` stubbed to
 * return a fixed address for a fixed signature string, issue and verify could serialise
 * the credential completely differently and every assertion still passed (plan §0.5 B).
 * The `with real ethers` block at the bottom flips this to `true` and is the only thing
 * in this file that can observe a signature at all.
 *
 * A mutable object rather than a boolean: `jest.mock` factories are hoisted above every
 * declaration in the file, so the closures below must dereference at CALL time.
 */
const mockEthersMode = { real: false };

// Mock ethers v6
jest.mock('ethers', () => {
  const actual = jest.requireActual('ethers');

  return {
    verifyMessage: jest.fn((message, signature) => {
      if (mockEthersMode.real) {
        return actual.verifyMessage(message, signature);
      }
      // For valid DID signature
      if (signature === 'valid_did_signature') {
        return '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      }
      // For valid issuer signature
      if (signature === 'valid_issuer_signature') {
        return '0x1234567890123456789012345678901234567890';
      }
      // For invalid cases, return a different address
      return '0x1111111111111111111111111111111111111111';
    }),
    // A constructor function, not a class, so it can *return* a real Wallet when the
    // real mode is on; `new` honours an explicit object return.
    Wallet: function (privateKey) {
      if (mockEthersMode.real) {
        return new actual.Wallet(privateKey);
      }
      return {
        signMessage: () => Promise.resolve('valid_issuer_signature'),
      };
    },
  };
});

// Mock Keys from @ew-did-registry/keys.
// `getAddress` derives the REAL address of the configured private key rather than
// returning a canned one, so that in real mode the issuer DID on a credential matches the
// key that signed it. The older cases do not assert on the issuer DID.
jest.mock('@ew-did-registry/keys', () => {
  const actual = jest.requireActual('ethers');

  return {
    Keys: jest.fn().mockImplementation(({ privateKey }: any) => ({
      privateKey,
      publicKey: '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc',
      getAddress: jest.fn(() => new actual.Wallet(privateKey).address),
    })),
  };
});

describe('CredentialsService', () => {
  let service: CredentialsService;
  let didService: DIDService;
  let substrateSigVerify: any;
  
  let mockCredentialModel: any;
  let mockUserModel: any;
  let mockAuditService: any;
  let mockConfigService: any;

  beforeEach(async () => {
    // Get the mocked substrate verification functions
    const subVerify = require('../src/credentials/utils/substrate-verification');
    substrateSigVerify = subVerify.verifySubstrateSignature;
    
    // Create mock Credential model.
    // A `jest.fn` rather than a plain function so a test can assert on WHAT was persisted
    // (`new this.credentialModel({...})` records its argument); `new` returns the object
    // the implementation returns.
    mockCredentialModel = jest.fn(function() {
      return {
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 5000), // 5 year from now
        save: jest.fn().mockResolvedValue(true),
      };
    });
    
    mockCredentialModel.findOne = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 5000), // 5 year from now
        save: jest.fn().mockResolvedValue(true),
      }),
    });
    
    mockCredentialModel.find = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue([{
        id: 'urn:uuid:test-credential-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        credentialSubject: {},
        credential: {},
        status: CredentialStatus.ACTIVE,
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000),
      }]),
    });
    
    // Create mock User model
    mockUserModel = function() {
      return {
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        hasVerifiedCredential: true,
        save: jest.fn().mockResolvedValue(true),
      };
    };
    
    mockUserModel.findOneAndUpdate = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
        hasVerifiedCredential: true,
      }),
    });
    
    // Create mock Audit service
    mockAuditService = {
      log: jest.fn().mockResolvedValue(true),
    };
    
    // Create mock Config service
    mockConfigService = {
      get: jest.fn().mockImplementation((key) => {
        if (key === 'ewc.issuerPrivateKey') {
          return ISSUER_PRIVATE_KEY;
        }
        if (key === 'ewc.issuerPublicKey') {
          return '02963497c702612b675707c0757e82b93df912261cd06f6a51e6c5419ac1aa9bcc';
        }
        return null;
      }),
    };
    
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        CredentialsService,
        {
          provide: ConfigService,
          useValue: mockConfigService,
        },
        {
          provide: DIDService,
          useValue: {
            isDIDRegistered: jest.fn().mockResolvedValue(true),
          },
        },
        {
          provide: getModelToken(Credential.name),
          useValue: mockCredentialModel,
        },
        {
          provide: getModelToken(User.name),
          useValue: mockUserModel,
        },
        {
          provide: AuditService,
          useValue: mockAuditService,
        },
      ],
    }).compile();

    service = module.get<CredentialsService>(CredentialsService);
    didService = module.get<DIDService>(DIDService);
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('issueCredential', () => {
    it('should issue a credential with valid signatures', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      const result = await service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      );
      
      expect(result).toBeDefined();
      expect(result.id).toBeDefined();
      expect(result.credential).toBeDefined();
      expect(result.credential['@context']).toContain('https://www.w3.org/2018/credentials/v1');
      expect(result.credential.credentialSubject.id).toBe(did);
      expect(result.credential.credentialSubject.accountLink.gsyDexAddress).toBe(gsyDexAddress);
      expect(mockAuditService.log).toHaveBeenCalled();
      expect(substrateSigVerify).toHaveBeenCalled();
    });

    it('should throw an error for unregistered DID', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      // Mock DID service to return false for unregistered DID
      jest.spyOn(didService, 'isDIDRegistered').mockResolvedValueOnce(false);
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(BadRequestException);
    });

    it('should throw an error for invalid DID signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'invalid_signature';
      const substrateSignature = 'valid_substrate_signature';
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(UnauthorizedException);
      
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_ISSUED,
        did,
        undefined,  // req parameter is undefined in test
        expect.objectContaining({ 
          gsyDexAddress,
          error: 'Invalid DID signature'
        }),
        gsyDexAddress,
        false,
      );
    });

    it('should throw an error for invalid Substrate signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      const challenge = 'test-challenge';
      const didSignature = 'valid_did_signature';
      const substrateSignature = 'invalid_substrate_signature';
      
      await expect(service.issueCredential(
        did,
        gsyDexAddress,
        challenge,
        didSignature,
        substrateSignature,
      )).rejects.toThrow(UnauthorizedException);
      
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_ISSUED,
        did,
        undefined,
        expect.objectContaining({ 
          gsyDexAddress,
          error: 'Invalid Substrate signature'
        }),
        gsyDexAddress,
        false,
      );
      expect(substrateSigVerify).toHaveBeenCalled();
    });
  });
  
  describe('issueAssetCredential (plan §2.7)', () => {
    const assetDid = 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21';
    const claims = {
      subjectUuid: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d',
      communityName: 'Pilot1',
      communityUuid: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
      assetName: 'LIC08SM',
      assetType: 'SMART_METER',
    };

    it('issues a FedecomAssetCredential carrying every ontology claim', async () => {
      const result = await service.issueAssetCredential(assetDid, claims);

      expect(result.credential.type).toEqual(['VerifiableCredential', 'FedecomAssetCredential']);
      expect(result.credential.credentialSubject).toEqual({ id: assetDid, ...claims });
      expect(result.credential.proof.proofPurpose).toBe('assertionMethod');
      expect(result.id).toMatch(/^urn:uuid:/);
    });

    it('does NOT require the DID to be registered on chain', async () => {
      // Phase 1 writes nothing on-chain and a did:ethr resolves without any registry
      // transaction, so gating on registration would gate credentials on phase 3.
      jest.spyOn(didService, 'isDIDRegistered').mockResolvedValue(false);

      await expect(service.issueAssetCredential(assetDid, claims)).resolves.toBeDefined();
      expect(didService.isDIDRegistered).not.toHaveBeenCalled();
    });

    it('requires no holder signature of any kind', async () => {
      // An asset cannot sign: it has no Substrate account, and a challenge signed by the
      // server with the server's own key would be evidence of nothing.
      // The two signature-verification mocks live in module scope, so clear them here
      // rather than trusting that no earlier case in this file used them.
      const ethersMock = require('ethers');
      substrateSigVerify.mockClear();
      ethersMock.verifyMessage.mockClear();

      await service.issueAssetCredential(assetDid, claims);

      expect(substrateSigVerify).not.toHaveBeenCalled();
      // No challenge recovery either: nothing on the holder side is checked, because
      // nothing on the holder side is claimed.
      expect(ethersMock.verifyMessage).not.toHaveBeenCalled();
    });

    it('stores the credential without inventing a gsyDexAddress', async () => {
      await service.issueAssetCredential(assetDid, claims);

      const persisted = mockCredentialModel.mock.calls[0][0];
      expect(persisted.did).toBe(assetDid);
      expect(persisted.status).toBe(CredentialStatus.ACTIVE);
      expect(persisted.gsyDexAddress).toBeUndefined();
      expect('gsyDexAddress' in persisted).toBe(false);
    });

    it('never creates or touches a User record for the asset', async () => {
      // An asset must not become an authenticated principal (plan §2.5).
      await service.issueAssetCredential(assetDid, claims);

      expect(mockUserModel.findOneAndUpdate).not.toHaveBeenCalled();
    });

    it('audit-logs ASSET_CREDENTIAL_ISSUED against the asset DID', async () => {
      const result = await service.issueAssetCredential(assetDid, claims);

      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.ASSET_CREDENTIAL_ISSUED,
        assetDid,
        undefined,
        expect.objectContaining({
          credentialId: result.id,
          subjectUuid: claims.subjectUuid,
        }),
      );
    });

    it('omits absent asset-only claims instead of signing an undefined', async () => {
      const communityClaims = {
        subjectUuid: claims.communityUuid,
        communityName: claims.communityName,
        communityUuid: claims.communityUuid,
        assetName: undefined,
        assetType: undefined,
      };

      const result = await service.issueAssetCredential(assetDid, communityClaims);

      expect('assetName' in result.credential.credentialSubject).toBe(false);
      expect('assetType' in result.credential.credentialSubject).toBe(false);
    });

    it('rejects a non-did:ethr subject', async () => {
      await expect(service.issueAssetCredential('0xdeadbeef', claims)).rejects.toThrow(
        BadRequestException,
      );
    });

    it('rejects incomplete claims rather than issuing a half-empty attestation', async () => {
      await expect(
        service.issueAssetCredential(assetDid, { subjectUuid: 'x' } as any),
      ).rejects.toThrow(BadRequestException);
    });
  });

  describe('verifyCredential', () => {
    it('should verify a valid credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date().toISOString(),
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(),
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
            chain: 'GSYDex',
          },
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date().toISOString(),
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          proofPurpose: 'assertionMethod',
          jws: 'valid_issuer_signature',
        },
      };
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(true);
      expect(result.did).toBe('did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93');
      expect(result.gsyDexAddress).toBe('5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN');
      expect(mockAuditService.log).toHaveBeenCalled();
    });

    it('should return invalid for revoked credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date().toISOString(),
        expirationDate: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(),
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          }
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date().toISOString(),
          proofPurpose: 'assertionMethod',
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          jws: 'valid_issuer_signature'
        }
      };
      
      // Mock credentialModel.findOne to return a revoked credential
      mockCredentialModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce({
          id: 'urn:uuid:test-credential-id',
          did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          status: CredentialStatus.REVOKED,
        }),
      });
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(false);
      expect(result.details.status).toBe('revoked');
    });

    it('should return invalid for expired credential', async () => {
      const credential = {
        id: 'urn:uuid:test-credential-id',
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'GSYDexAddressCredential'],
        issuer: 'did:ethr:0x1234567890123456789012345678901234567890',
        issuanceDate: new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString(),
        expirationDate: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(), // Expired 1 day ago
        credentialSubject: {
          id: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
          accountLink: {
            gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          }
        },
        proof: {
          type: 'EcdsaSecp256k1Signature2019',
          created: new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString(),
          proofPurpose: 'assertionMethod',
          verificationMethod: 'did:ethr:0x1234567890123456789012345678901234567890#controller',
          jws: 'valid_issuer_signature'
        }
      };
      
      const result = await service.verifyCredential(credential);
      
      expect(result).toBeDefined();
      expect(result.valid).toBe(false);
      expect(result.details.status).toBe('expired');
    });
  });

  describe('revokeCredential', () => {
    it('should revoke a valid credential', async () => {
      const id = 'urn:uuid:test-credential-id';
      
      const result = await service.revokeCredential(id);
      
      expect(result).toBe(true);
      expect(mockAuditService.log).toHaveBeenCalledWith(
        AuditAction.CREDENTIAL_REVOKED,
        expect.any(String),
        undefined,  // req parameter is undefined in test
        expect.objectContaining({ credentialId: id }),
        expect.any(String),
      );
    });

    it('should throw an error for non-existent credential', async () => {
      const id = 'urn:uuid:non-existent-id';
      
      // Mock credentialModel.findOne to return null for non-existent credential
      mockCredentialModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce(null),
      });
      
      await expect(service.revokeCredential(id)).rejects.toThrow(BadRequestException);
    });
  });

  /**
   * The only cases in this file that observe a real signature.
   *
   * Everything above runs against a `verifyMessage` stub, which is precisely why plan
   * §0.5 bug (B) went unnoticed: the issue path and the verify path serialised the
   * credential differently for as long as the service has existed, and no assertion here
   * could tell. These cases turn the stub off (`mockEthersMode.real`) and use the real
   * library at both ends.
   *
   * Two criteria, and the second is the one that matters (plan §3, phase 4):
   *   1. issue -> verify round-trips to `valid: true`;
   *   2. mutating a NESTED `credentialSubject` field makes it `valid: false`.
   * Criterion 1 alone is satisfied by a canonicaliser that strips the claims - which is
   * what "align both ends on the sorted-replacer form" would have produced. Only the
   * tamper case distinguishes a signature over the credential from a signature over the
   * subject DID alone.
   */
  describe('with real ethers (plan §0.5 bug B)', () => {
    const realEthers = jest.requireActual('ethers');

    const holderWallet = new realEthers.Wallet(
      '0x2222222222222222222222222222222222222222222222222222222222222222',
    );
    const holderDid = `did:ethr:${holderWallet.address}`;
    const gsyDexAddress = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
    const challenge = `Link GSY DEX address ${gsyDexAddress} to DID ${holderDid}`;

    beforeEach(() => {
      mockEthersMode.real = true;
    });

    afterEach(() => {
      mockEthersMode.real = false;
    });

    /** Issues through the real path: real holder signature, real issuer signature. */
    const issue = async () => {
      const didSignature = await holderWallet.signMessage(challenge);

      const result = await service.issueCredential(
        holderDid,
        gsyDexAddress,
        challenge,
        didSignature,
        'valid_substrate_signature', // substrate crypto stays mocked; only ethers is real
      );

      // The stored record is what `verifyCredential` looks up. Point the mock at the
      // credential just issued so the lookup, the status and the expiry all line up.
      mockCredentialModel.findOne = jest.fn().mockReturnValue({
        exec: jest.fn().mockResolvedValue({
          id: result.id,
          did: holderDid,
          gsyDexAddress,
          credential: result.credential,
          status: CredentialStatus.ACTIVE,
          expirationDate: new Date(result.credential.expirationDate),
        }),
      });

      return result;
    };

    it('issues a credential whose issuer DID matches the signing key', async () => {
      const { credential } = await issue();

      expect(credential.issuer).toBe(`did:ethr:${new realEthers.Wallet(ISSUER_PRIVATE_KEY).address}`);
      expect(credential.proof.jws).toMatch(/^0x[0-9a-f]{130}$/i);
    });

    it('round-trips: a credential it issues verifies as valid (defect 1)', async () => {
      const { credential } = await issue();

      const result = await service.verifyCredential(credential);

      expect(result.valid).toBe(true);
      expect(result.details.signature).toBe('valid');
      expect(result.did).toBe(holderDid);
    });

    it('still verifies after a JSON round trip that shuffles key order', async () => {
      // The production path: the credential comes back from Mongo and over HTTP, so its
      // key order is not the insertion order that was signed. This is the case the
      // canonicaliser exists for.
      const { credential } = await issue();

      const shuffled = JSON.parse(
        JSON.stringify({
          proof: credential.proof,
          credentialSubject: {
            accountLink: {
              chain: credential.credentialSubject.accountLink.chain,
              gsyDexAddress: credential.credentialSubject.accountLink.gsyDexAddress,
            },
            id: credential.credentialSubject.id,
          },
          expirationDate: credential.expirationDate,
          issuanceDate: credential.issuanceDate,
          issuer: credential.issuer,
          type: credential.type,
          id: credential.id,
          '@context': credential['@context'],
        }),
      );

      const result = await service.verifyCredential(shuffled);

      expect(result.valid).toBe(true);
    });

    it('rejects a credential whose NESTED credentialSubject claim was flipped (defect 2)', async () => {
      // THE control for a claim-stripping canonicaliser. `gsyDexAddress` lives two levels
      // down, so under the old array-replacer serialisation it was never part of the
      // signed bytes and this tamper verified happily as valid.
      const { credential } = await issue();

      const tampered = JSON.parse(JSON.stringify(credential));
      tampered.credentialSubject.accountLink.gsyDexAddress =
        '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty';

      const result = await service.verifyCredential(tampered);

      expect(result.valid).toBe(false);
      expect(result.details.status).toBe('invalid');
      expect(result.details.reason).toBe('Invalid signature');
    });

    it('rejects a credential whose nested chain claim was flipped', async () => {
      const { credential } = await issue();

      const tampered = JSON.parse(JSON.stringify(credential));
      tampered.credentialSubject.accountLink.chain = 'SomeOtherChain';

      expect((await service.verifyCredential(tampered)).valid).toBe(false);
    });

    it('rejects a credential whose subject DID was swapped', async () => {
      const { credential } = await issue();

      const tampered = JSON.parse(JSON.stringify(credential));
      tampered.credentialSubject.id = 'did:ethr:0x1111111111111111111111111111111111111111';

      expect((await service.verifyCredential(tampered)).valid).toBe(false);
    });

    it('round-trips an asset credential, and its nested claims are covered too', async () => {
      const assetDid = 'did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21';
      const claims = {
        subjectUuid: '1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d',
        communityName: 'Pilot1',
        communityUuid: 'c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c',
        assetName: 'LIC08SM',
        assetType: 'SMART_METER',
      };

      const issued = await service.issueAssetCredential(assetDid, claims);

      mockCredentialModel.findOne = jest.fn().mockReturnValue({
        exec: jest.fn().mockResolvedValue({
          id: issued.id,
          did: assetDid,
          credential: issued.credential,
          status: CredentialStatus.ACTIVE,
          expirationDate: new Date(issued.credential.expirationDate),
        }),
      });

      // Criterion 3: it verifies as issued.
      const valid = await service.verifyCredential(JSON.parse(JSON.stringify(issued.credential)));
      expect(valid.valid).toBe(true);

      // Criterion 2 for the asset shape: EVERY subject field of a FedecomAssetCredential
      // is a nested key with no top-level namesake, so all of them were dropped by the old
      // replacer form. Flipping any one of them must now invalidate the signature.
      for (const field of ['subjectUuid', 'assetName', 'assetType', 'communityName', 'communityUuid']) {
        const tampered = JSON.parse(JSON.stringify(issued.credential));
        tampered.credentialSubject[field] = 'tampered';

        const result = await service.verifyCredential(tampered);
        expect([field, result.valid]).toEqual([field, false]);
      }
    });

    it('rejects a credential whose expiry was pushed out', async () => {
      const { credential } = await issue();

      const tampered = JSON.parse(JSON.stringify(credential));
      tampered.expirationDate = new Date(Date.now() + 10 * 365 * 24 * 60 * 60 * 1000).toISOString();

      expect((await service.verifyCredential(tampered)).valid).toBe(false);
    });
  });

  describe('getCredentialsByDid', () => {
    it('should return credentials for a valid DID', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      
      const result = await service.getCredentialsByDid(did);
      
      expect(result).toBeDefined();
      expect(Array.isArray(result)).toBe(true);
      expect(result.length).toBeGreaterThan(0);
      expect(result[0].did).toBe(did);
    });

    it('should return empty array for DID with no credentials', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      
      // Mock credentialModel.find to return empty array
      mockCredentialModel.find.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce([]),
      });
      
      const result = await service.getCredentialsByDid(did);
      
      expect(result).toBeDefined();
      expect(Array.isArray(result)).toBe(true);
      expect(result.length).toBe(0);
    });
  });
});