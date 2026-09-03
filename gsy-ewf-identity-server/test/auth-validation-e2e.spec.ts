import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication } from '@nestjs/common';
import request from 'supertest';
import { JwtService } from '@nestjs/jwt';
import { getModelToken } from '@nestjs/mongoose';
import { AppModule } from '../src/app.module';
import { User } from '../src/database/schemas/user.schema';
import { Model } from 'mongoose';

describe('Auth Validation (e2e)', () => {
  let app: INestApplication;
  let jwtService: JwtService;
  let userModel: Model<User>;
  
  // Test user that should exist in the database
  const testUser = { did: 'did:ethr:0x5a915Fd0B025d20eD0D1Ae83877208fA50Cd6B84' };
  let testUserToken: string;

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule],
    }).compile();

    app = moduleFixture.createNestApplication();
    await app.init();

    // Get the JWT service
    jwtService = app.get<JwtService>(JwtService);
    testUserToken = jwtService.sign({ sub: testUser.did });
    
    try {
      // Get the user model
      userModel = app.get<Model<User>>(getModelToken(User.name));
      
      // Create test user in database if it doesn't exist
      const existingUser = await userModel.findOne({ did: testUser.did }).exec();
      if (!existingUser) {
        await userModel.create({ did: testUser.did });
      } else {
        console.log('Test user already exists in database');
      }
    } catch (error) {
      throw error;
    }
  });

  afterAll(async () => {
    if (app) {
      await app.close();
    }
  });

  it('should check if test user exists in database', async () => {
    try {
      const user = await userModel.findOne({ did: testUser.did }).exec();
      expect(user).toBeDefined();
    } catch (error) {
      throw error;
    }
  });

  it('should authenticate with a valid token for existing user', async () => {
    // Access a protected endpoint with the token
    return request(app.getHttpServer())
      .get('/credentials/my')
      .set('Authorization', `Bearer ${testUserToken}`)
      .expect(200);
  });
});