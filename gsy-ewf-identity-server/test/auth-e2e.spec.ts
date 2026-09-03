import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication, ValidationPipe } from '@nestjs/common';
import request from 'supertest';
import { JwtService } from '@nestjs/jwt';
import { getModelToken } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { AppModule } from '../src/app.module';
import { User } from '../src/database/schemas/user.schema';
import { Credential, CredentialStatus } from '../src/database/schemas/credential.schema';
import { v4 as uuidv4 } from 'uuid';
import { Wallet } from 'ethers';
import { UserInfoDto } from '../src/auth/dto/user-info.dto';

describe('Authorization (e2e)', () => {
  let app: INestApplication;
  let jwtService: JwtService;
  let userModel: Model<User>;
  let credentialModel: Model<Credential>;
  let ownerUserDto: UserInfoDto;
  
  const ownerUser = { did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93' };
  const otherUser = { did: 'did:ethr:0x1111111111111111111111111111111111111111' };
  let ownerToken: string;
  let otherToken: string;

  const ownerCredentialId = `urn:uuid:${uuidv4()}`;
  const otherCredentialId = `urn:uuid:${uuidv4()}`;

  jest.setTimeout(60000);

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule],
    }).compile();

    app = moduleFixture.createNestApplication();
    app.useGlobalPipes(new ValidationPipe());
    await app.init();

    jwtService = app.get<JwtService>(JwtService);
    userModel = app.get<Model<User>>(getModelToken(User.name));
    credentialModel = app.get<Model<Credential>>(getModelToken(Credential.name));

    ownerToken = jwtService.sign({ sub: ownerUser.did });
    otherToken = jwtService.sign({ sub: otherUser.did });

    await setupTestData();
    const userDoc = await userModel.findOne({ did: ownerUser.did }).lean().exec();
    if (!userDoc) throw new Error("Setup failed: Owner user not found after setup.");
    ownerUserDto = {
      did: userDoc.did,
      gsyDexAddress: userDoc.gsyDexAddress,
      hasVerifiedCredential: userDoc.hasVerifiedCredential,
      metadata: userDoc.metadata
    };
  });

  afterAll(async () => {
    await cleanupTestData();
    await app.close();
  });

  async function setupTestData() {
    try {
      await userModel.findOneAndUpdate(
        { did: ownerUser.did },
        { hasVerifiedCredential: true },
        { upsert: true, new: true }
      ).exec();

      await userModel.findOneAndUpdate(
        { did: otherUser.did },
        { hasVerifiedCredential: true },
        { upsert: true, new: true }
      ).exec();

      await credentialModel.findOneAndUpdate(
        { id: ownerCredentialId },
        {
          id: ownerCredentialId,
          did: ownerUser.did,
          gsyDexAddress: '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN',
          credentialSubject: { id: ownerUser.did },
          credential: { type: ['VerifiableCredential'] },
          status: CredentialStatus.ACTIVE,
          expirationDate: new Date(Date.now() + 3600 * 1000)
        },
        { upsert: true, new: true }
      ).exec();

      await credentialModel.findOneAndUpdate(
        { id: otherCredentialId },
        {
          id: otherCredentialId,
          did: otherUser.did,
          gsyDexAddress: '5H9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBHTS9LM2FPTN',
          credentialSubject: { id: otherUser.did },
          credential: { type: ['VerifiableCredential'] },
          status: CredentialStatus.ACTIVE,
          expirationDate: new Date(Date.now() + 3600 * 1000)
        },
        { upsert: true, new: true }
      ).exec();

    } catch (error) {
      throw error;
    }
  }

  async function cleanupTestData() {
    try {
      await credentialModel.deleteOne({ id: ownerCredentialId }).exec();
      await credentialModel.deleteOne({ id: otherCredentialId }).exec();
      
      await userModel.deleteOne({ did: ownerUser.did }).exec();
      await userModel.deleteOne({ did: otherUser.did }).exec();
      
    } catch (error) {
      throw error;
    }
  }

  describe('DID Owner Authorization', () => {
    it('should allow access to own DID data', async () => {
      const response = await request(app.getHttpServer())
        .get(`/did/${ownerUser.did}/exists`)
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(200);
      
      expect(response.body).toHaveProperty('registered');
    });

    it('should allow preparing update transaction for own DID', async () => {
      const updatePayload = { publicKey: Wallet.createRandom().publicKey };
      const response = await request(app.getHttpServer())
        .post(`/did/${ownerUser.did}/prepare-update`) 
        .set('Authorization', `Bearer ${ownerToken}`)
        .send(updatePayload)
        .expect(200); 

      expect(response.body).toHaveProperty('to');
      expect(response.body).toHaveProperty('data');
      expect(response.body.data).toMatch(/^0x[0-9a-fA-F]+$/); 
    });

    it('should deny preparing update transaction for another user\'s DID', async () => {
      const updatePayload = { publicKey: Wallet.createRandom().publicKey };
      await request(app.getHttpServer())
        .post(`/did/${otherUser.did}/prepare-update`) 
        .set('Authorization', `Bearer ${ownerToken}`) 
        .send(updatePayload)
        .expect(403); 
    });

    it('should deny preparing update transaction without authentication', async () => {
      const updatePayload = { publicKey: Wallet.createRandom().publicKey };
      await request(app.getHttpServer())
        .post(`/did/${ownerUser.did}/prepare-update`) 
        .send(updatePayload)
        .expect(401); 
    });
  });

  describe('Credential Owner Authorization', () => {
    it('should allow access to own credentials by DID', async () => {
      const response = await request(app.getHttpServer())
        .get(`/credentials/did/${ownerUser.did}`)
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(200);
      
      expect(Array.isArray(response.body)).toBe(true);
    });

    it('should deny access to another user\'s credentials by DID', async () => {
      await request(app.getHttpServer())
        .get(`/credentials/did/${otherUser.did}`)
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(403); 
    });

    it('should allow revoking own credential', async () => {
      await request(app.getHttpServer())
        .delete(`/credentials/${ownerCredentialId}`)
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(200)
        .expect({ success: true });
    });

    it('should deny revoking another user\'s credential', async () => {
      await request(app.getHttpServer())
        .delete(`/credentials/${otherCredentialId}`)
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(403); 
    });
  });

  describe('Token Verification Endpoint (/auth/verify-token)', () => {
    it('should return user info for a valid token', async () => {
      const response = await request(app.getHttpServer())
        .get('/auth/verify-token')
        .set('Authorization', `Bearer ${ownerToken}`) 
        .expect(200);

      expect(response.body).toBeDefined();
      expect(response.body.did).toEqual(ownerUserDto.did);
      expect(response.body.gsyDexAddress).toEqual(ownerUserDto.gsyDexAddress);
      expect(response.body.hasVerifiedCredential).toEqual(ownerUserDto.hasVerifiedCredential);
    });

    it('should return 401 Unauthorized for no token', async () => {
      await request(app.getHttpServer())
        .get('/auth/verify-token')
        .expect(401);
    });

    it('should return 401 Unauthorized for an invalid/malformed token', async () => {
      await request(app.getHttpServer())
        .get('/auth/verify-token')
        .set('Authorization', `Bearer invalid.token.string`)
        .expect(401);
    });

    it('should return 401 Unauthorized for an expired token', async () => {
      const expiredToken = jwtService.sign({ sub: ownerUser.did }, { expiresIn: '-1s' });

      await request(app.getHttpServer())
        .get('/auth/verify-token')
        .set('Authorization', `Bearer ${expiredToken}`)
        .expect(401);
    });

     it('should return 401 Unauthorized if the user associated with token no longer exists', async () => {
      const tempDid = `did:ethr:${Wallet.createRandom().address.toLowerCase()}`;
      await userModel.create({ did: tempDid });
      const tempToken = jwtService.sign({ sub: tempDid });

      await request(app.getHttpServer())
        .get('/auth/verify-token')
        .set('Authorization', `Bearer ${tempToken}`)
        .expect(200);

      await userModel.deleteOne({ did: tempDid }).exec();

      await request(app.getHttpServer())
          .get('/auth/verify-token')
          .set('Authorization', `Bearer ${tempToken}`)
          .expect(401);
    });
  });

  describe('Convenience Endpoints', () => {
    it('should allow access to my credentials endpoint', async () => {
      const response = await request(app.getHttpServer())
        .get('/credentials/my')
        .set('Authorization', `Bearer ${ownerToken}`)
        .expect(200);
      
      expect(Array.isArray(response.body)).toBe(true);
    });
  });
});