import { Test, TestingModule } from '@nestjs/testing';
import { JwtService } from '@nestjs/jwt';
import { getModelToken } from '@nestjs/mongoose';
import { BadRequestException, UnauthorizedException } from '@nestjs/common';
import { AuthService } from '../src/auth/auth.service';
import { DIDService } from '../src/did/did.service';
import { AuditService } from '../src/audit/audit.service';
import { Challenge } from '../src/database/schemas/challenge.schema';
import { User } from '../src/database/schemas/user.schema';

// Mock ethers v6
jest.mock('ethers', () => ({
  verifyMessage: jest.fn((message, signature) => {
    // For valid test cases, return the address in the DID
    if (signature === 'valid_signature') {
      return '0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
    }
    // For invalid cases, return a different address
    return '0x1111111111111111111111111111111111111111';
  })
}));

describe('AuthService', () => {
  let service: AuthService;
  let didService: DIDService;
  let jwtService: JwtService;
  
  let mockChallengeModel: any;
  let mockUserModel: any;
  let mockAuditService: any;

  beforeEach(async () => {
    // Create mock Challenge model
    mockChallengeModel = function() {
      return {
        id: 'test-challenge-id',
        challenge: 'Sign this message to authenticate: test-challenge-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        timestamp: new Date(),
        used: false,
        save: jest.fn().mockResolvedValue(true),
      };
    };
    
    mockChallengeModel.findOne = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        id: 'test-challenge-id',
        challenge: 'Sign this message to authenticate: test-challenge-id',
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        timestamp: new Date(),
        used: false,
        save: jest.fn().mockResolvedValue(true),
      }),
    });
    
    // Create mock User model
    mockUserModel = function() {
      return {
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        save: jest.fn().mockResolvedValue(true),
      };
    };
    
    mockUserModel.findOne = jest.fn().mockReturnValue({
      exec: jest.fn().mockResolvedValue({
        did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        toObject: jest.fn().mockReturnValue({
          did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93',
        }),
      }),
    });
    
    // Create mock Audit service
    mockAuditService = {
      log: jest.fn().mockResolvedValue(true),
    };
    
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AuthService,
        {
          provide: JwtService,
          useValue: {
            sign: jest.fn().mockReturnValue('test-jwt-token'),
          },
        },
        {
          provide: DIDService,
          useValue: {
            isDIDRegistered: jest.fn().mockResolvedValue(true),
          },
        },
        {
          provide: getModelToken(Challenge.name),
          useValue: mockChallengeModel,
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

    service = module.get<AuthService>(AuthService);
    didService = module.get<DIDService>(DIDService);
    jwtService = module.get<JwtService>(JwtService);
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('generateChallenge', () => {
    it('should generate a challenge for a registered DID', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      
      const result = await service.generateChallenge(did);
      
      expect(result).toBeDefined();
      expect(result.id).toBeDefined();
      expect(result.challenge).toContain(result.id);
      expect(result.timestamp).toBeDefined();
      expect(mockAuditService.log).toHaveBeenCalled();
    });

    it('should throw an error for an unregistered DID', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      
      // Mock DID service to return false for unregistered DID
      jest.spyOn(didService, 'isDIDRegistered').mockResolvedValueOnce(false);
      
      await expect(service.generateChallenge(did)).rejects.toThrow(BadRequestException);
    });
  });

  describe('verifyChallenge', () => {
    it('should verify a valid challenge and signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const challengeId = 'test-challenge-id';
      const signature = 'valid_signature';
      
      const result = await service.verifyChallenge(did, challengeId, signature);
      
      expect(result).toBeDefined();
      expect(result.accessToken).toBe('test-jwt-token');
      expect(result.did).toBe(did);
      expect(mockAuditService.log).toHaveBeenCalledWith(
        expect.anything(),
        did,
        null,
        expect.objectContaining({ challengeId }),
      );
    });

    it('should throw an error for an invalid challenge', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const challengeId = 'invalid-challenge-id';
      const signature = 'valid_signature';
      
      // Mock challengeModel.findOne to return null for invalid challenge
      mockChallengeModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce(null),
      });
      
      await expect(service.verifyChallenge(did, challengeId, signature)).rejects.toThrow(BadRequestException);
    });

    it('should throw an error for an invalid signature', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      const challengeId = 'test-challenge-id';
      const signature = 'invalid_signature';
      
      await expect(service.verifyChallenge(did, challengeId, signature)).rejects.toThrow(UnauthorizedException);
      expect(mockAuditService.log).toHaveBeenCalledWith(
        expect.anything(),
        did,
        null,
        expect.objectContaining({ 
          challengeId,
          error: expect.any(String)
        }),
        null,
        false,
      );
    });
  });

  describe('validateUser', () => {
    it('should return user object for valid DID', async () => {
      const did = 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93';
      
      const result = await service.validateUser(did);
      
      expect(result).toBeDefined();
      expect(result.did).toBe(did);
    });

    it('should return null for invalid DID', async () => {
      const did = 'did:ethr:0x1111111111111111111111111111111111111111';
      
      // Mock userModel.findOne to return null for invalid DID
      mockUserModel.findOne.mockReturnValueOnce({
        exec: jest.fn().mockResolvedValueOnce(null),
      });
      
      const result = await service.validateUser(did);
      
      expect(result).toBeNull();
    });
  });
});