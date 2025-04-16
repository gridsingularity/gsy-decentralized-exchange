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

describe('Authorization (e2e)', () => {
  let app: INestApplication;
  let jwtService: JwtService;
  let userModel: Model<User>;
  let credentialModel: Model<Credential>;
  
  // Test users
  const ownerUser = { did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B93' };
  const otherUser = { did: 'did:ethr:0x1111111111111111111111111111111111111111' };
  let ownerToken: string;
  let otherToken: string;

  // Test credentials
  const ownerCredentialId = `urn:uuid:${uuidv4()}`;
  const otherCredentialId = `urn:uuid:${uuidv4()}`;

  // Set a higher timeout for all tests
  jest.setTimeout(60000);

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule],
    }).compile();

    app = moduleFixture.createNestApplication();
    app.useGlobalPipes(new ValidationPipe());
    await app.init();

    // Get services and models
    jwtService = app.get<JwtService>(JwtService);
    userModel = app.get<Model<User>>(getModelToken(User.name));
    credentialModel = app.get<Model<Credential>>(getModelToken(Credential.name));

    // Create test tokens
    ownerToken = jwtService.sign({ sub: ownerUser.did });
    otherToken = jwtService.sign({ sub: otherUser.did });

    // Setup test data in the database
    await setupTestData();
  });

  afterAll(async () => {
    // Clean up test data
    await cleanupTestData();
    await app.close();
  });

  // Helper to set up test data
  async function setupTestData() {
    try {
      // Create or update test users
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

      // Create test credentials
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

  // Helper to clean up test data
  async function cleanupTestData() {
    try {
      // Remove test data
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
        .expect(403); // DIDOwnerGuard should return Forbidden
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
        .expect(403); // Forbidden
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