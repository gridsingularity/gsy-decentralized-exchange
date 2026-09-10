import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication, ValidationPipe } from '@nestjs/common';
import request from 'supertest';
import { JwtService } from '@nestjs/jwt';
import { getModelToken } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { ethers, ZeroAddress, Wallet, TransactionResponse, TransactionReceipt, Provider } from 'ethers';
import { User } from '../src/database/schemas/user.schema';
import { Credential } from '../src/database/schemas/credential.schema';
import { AppModule } from '../src/app.module';
import { ConfigService } from '@nestjs/config';
import { formatSubstrateSigningMessage } from '../src/credentials/utils/substrate-verification';
import {
  cryptoWaitReady,
  encodeAddress,
  sr25519PairFromSeed,
  sr25519Sign,
} from '@polkadot/util-crypto';
import { stringToU8a, u8aToHex } from '@polkadot/util';
import { PreparedTransactionDto } from '../src/did/dto/prepared-transaction.dto';

// ApiKeyGuard fails closed: AppModule refuses to boot without a configured API key
// (plan §2.6). These e2e specs build the whole AppModule, so give them one.
process.env.API_KEY = process.env.API_KEY || 'test_api_key';
// AssetKeyService likewise refuses to start without a valid master seed.
// Obviously-fake fixed test seed - never a real one.
process.env.ASSET_DID_MASTER_SEED =
  process.env.ASSET_DID_MASTER_SEED || '01'.repeat(32);

const ERC1056_ABI = [
  "function identityOwner(address identity) view returns (address)",
];

async function waitForTransaction(
  provider: Provider,
  txResponse: TransactionResponse,
  confirmations: number = 1,
  timeoutMs: number = 120000
): Promise<TransactionReceipt | null> {
    console.log(`Waiting for transaction ${txResponse.hash} (${confirmations} confirmations)...`);
    try {
        const receipt = await provider.waitForTransaction(txResponse.hash, confirmations, timeoutMs);
        if (!receipt) {
             console.warn(`TX ${txResponse.hash} receipt not found after timeout.`);
             return null;
        }
        console.log(`TX ${txResponse.hash} confirmed in block ${receipt.blockNumber}, Status: ${receipt.status}`);
        return receipt;
    } catch (error: any) {
        console.error(`Error waiting for TX ${txResponse.hash}:`, error.message);
        if (error.code === 'TIMEOUT') {
             console.error(`Timeout waiting for TX confirmation.`);
        }
        throw new Error(`Failed to wait for transaction ${txResponse.hash}: ${error.message}`);
    }
}


describe('DID and Credential Flow (e2e)', () => {
  let app: INestApplication;
  let configService: ConfigService;
  let userModel: Model<User>;
  let credentialModel: Model<Credential>;
  let jwtService: JwtService;

  let provider: ethers.JsonRpcProvider;
  let didRegistry: ethers.Contract;

  let issuerPrivateKey: string;
  let issuerWallet: ethers.Wallet;
  let issuerAddress: string;

  let testWallet: ethers.Wallet;
  let testAddress: string;
  let testDid: string;
  let testToken: string;

  /**
   * A REAL sr25519 keypair, not a placeholder address.
   *
   * The credential issuance route verifies the Substrate signature for real
   * (`credentials.service.ts:85-101`), so the only way to exercise the actual issuance
   * path is to hold the key for the address being linked. This used to be a hardcoded
   * address with a fabricated signature, which always 401'd and sent the test down a
   * hand-signing workaround that bypassed the service entirely (plan §2.7).
   *
   * Obviously-fake fixed seed - deterministic so a failure is reproducible.
   */
  const substrateSeed = new Uint8Array(32).fill(7);
  let substratePair: { publicKey: Uint8Array; secretKey: Uint8Array };
  let gsyDexAddress: string;

  let savedCredentialId: string;
  let didSuccessfullyRegistered = false;

  jest.setTimeout(300000);

  beforeAll(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule],
    }).compile();

    app = moduleFixture.createNestApplication();
    app.useGlobalPipes(new ValidationPipe());
    await app.init();

    jwtService = app.get<JwtService>(JwtService);
    configService = app.get<ConfigService>(ConfigService);
    userModel = app.get<Model<User>>(getModelToken(User.name));
    credentialModel = app.get<Model<Credential>>(getModelToken(Credential.name));

    const rpcUrl = configService.get<string>('ewc.rpcUrl');
    const didRegistryAddress = configService.get<string>('ewc.didRegistryAddress');
    issuerPrivateKey = configService.get<string>('ewc.issuerPrivateKey');

    if (!rpcUrl || !didRegistryAddress || !issuerPrivateKey) {
      throw new Error('Missing required EWC configuration');
    }

    provider = new ethers.JsonRpcProvider(rpcUrl);
    didRegistry = new ethers.Contract(didRegistryAddress, ERC1056_ABI, provider);
    issuerWallet = new ethers.Wallet(issuerPrivateKey, provider);
    issuerAddress = await issuerWallet.getAddress();

    await cryptoWaitReady();
    substratePair = sr25519PairFromSeed(substrateSeed);
    // 42 = the generic substrate prefix, which is what GSY DEX addresses use.
    gsyDexAddress = encodeAddress(substratePair.publicKey, 42);

    const randomWallet = ethers.Wallet.createRandom();
    testWallet = new ethers.Wallet(randomWallet.privateKey, provider);
    testAddress = await testWallet.getAddress();
    testDid = `did:ethr:${testAddress.toLowerCase()}`;
    console.log(`Test wallet created: ${testDid}`);

    const fundingAmountString = "0.00000000001";
    const fundingAmountWei = ethers.parseEther(fundingAmountString);
    console.log(`Attempting to fund test wallet ${testAddress} with ${fundingAmountString} VT...`);

    try {
        const tx = await issuerWallet.sendTransaction({
            to: testAddress,
            value: fundingAmountWei
        });
        const receipt = await waitForTransaction(provider, tx, 1, 180000);
        if (!receipt || receipt.status !== 1) {
             throw new Error(`Funding transaction ${tx.hash} failed or timed out.`);
        }
        const finalTestBalance = await provider.getBalance(testAddress);
        console.log(`Test wallet balance after funding: ${ethers.formatEther(finalTestBalance)} VT`);
        expect(finalTestBalance).toBeGreaterThanOrEqual(fundingAmountWei);

    } catch (error: any) {
        console.error(`Failed to fund test wallet:`, error);
        throw new Error(`Could not fund test wallet ${testAddress}. Error: ${error.message}`);
    }

  }, 240000);

  afterAll(async () => {
    try {
      if (testDid) {
          await userModel.deleteOne({ did: testDid }).exec();
          await credentialModel.deleteMany({ did: testDid }).exec();
      }
    } catch (error) {
      console.error('Error cleaning up test data:', error);
    }
    if (app) {
      await app.close();
    }
  }, 120000);

  describe('DID Registration and Authentication Flow', () => {
    it('should prepare initial DID transaction via API, execute it, and verify registration', async () => {
        didSuccessfullyRegistered = false;

        const initialOwner = await didRegistry.identityOwner(testAddress);
        expect(initialOwner.toLowerCase()).toEqual(testAddress.toLowerCase());
        const userBefore = await userModel.findOne({ did: testDid }).exec();
        expect(userBefore).toBeNull();

        let prepareTxResponse: request.Response;
        let preparedTx: PreparedTransactionDto;
        try {
            prepareTxResponse = await request(app.getHttpServer())
            .post('/did')
            // POST /did is behind ApiKeyGuard (plan §0.5 bug A / §2.6).
            .set('x-api-key', process.env.API_KEY)
            .send({
                address: testAddress.toLowerCase(),
                metadata: { name: 'E2E User TX Test', description: 'Preparing TX' }
            });

            expect(prepareTxResponse.status).toBe(200);
            expect(prepareTxResponse.body).toHaveProperty('to', didRegistry.target);
            expect(prepareTxResponse.body).toHaveProperty('data');
            preparedTx = prepareTxResponse.body;

        } catch (error: any) {
            console.error('Error preparing DID transaction via API:', error);
            if (prepareTxResponse!) console.error('Response Body:', prepareTxResponse!.body);
            throw error;
        }

        let txResponse: TransactionResponse | null = null;
        let txReceipt: TransactionReceipt | null = null;
        try {
            const tx = { to: preparedTx.to, data: preparedTx.data };
            txResponse = await testWallet.sendTransaction(tx);
            txReceipt = await waitForTransaction(provider, txResponse, 1, 120000);
            expect(txReceipt).toBeDefined();
            expect(txReceipt?.status).toBe(1);

        } catch (error: any) {
             console.error(`Error sending/confirming setAttribute transaction ${txResponse?.hash}:`, error);
             if (error.receipt) console.error("Transaction Reverted. Receipt:", error.receipt);
             throw error;
        }

        const registrationStatus = await request(app.getHttpServer())
           .get(`/did/${testDid}/exists`)
           .expect(200);
        expect(registrationStatus.body.registered).toBe(true);

        const finalOwnerOnChain = await didRegistry.identityOwner(testAddress);
        expect(finalOwnerOnChain.toLowerCase()).toEqual(testAddress.toLowerCase());

        const userAfter = await userModel.findOne({ did: testDid }).exec();
        expect(userAfter).toBeDefined();
        expect(userAfter?.did).toEqual(testDid);

        didSuccessfullyRegistered = true;
        console.log(`DID ${testDid} successfully registered and verified.`);
    });

    it('should generate an authentication challenge and get a token', async () => {
        expect(didSuccessfullyRegistered).toBe(true);
        try {
            const challengeResponse = await request(app.getHttpServer()).post('/auth/challenge').send({ did: testDid }).expect(200);
            const challenge = challengeResponse.body;
            const signature = await testWallet.signMessage(challenge.challenge);
            const verifyResponse = await request(app.getHttpServer()).post('/auth/verify').send({ did: testDid, challengeId: challenge.id, signature }).expect(200);
            testToken = verifyResponse.body.accessToken;
            expect(testToken).toBeDefined();
            expect(verifyResponse.body.did).toEqual(testDid);
        } catch (error: any) {
            console.error('Authentication Error:', error.response?.body || error.message);
            throw error;
        }
    });
  });

  describe('Credential Issuance Flow', () => {
    it('should issue a credential through the real issuance path', async () => {
        expect(didSuccessfullyRegistered).toBe(true);
        expect(testToken).toBeDefined();

        const issuerTxCountBefore = await provider.getTransactionCount(issuerAddress);

        const challenge = `Link GSY DEX address ${gsyDexAddress} to DID ${testDid} at ${new Date().toISOString()}`;
        const didSignature = await testWallet.signMessage(challenge);
        // Signed exactly as the service will re-derive it, with the <Bytes> wrapper a
        // browser wallet applies.
        const substrateMessage = formatSubstrateSigningMessage(challenge);
        const substrateSignature = u8aToHex(
            sr25519Sign(stringToU8a(substrateMessage), substratePair),
        );

        let issueResponse;
        try {
            issueResponse = await request(app.getHttpServer())
                .post('/credentials/issue')
                .set('Authorization', `Bearer ${testToken}`)
                .send({ did: testDid, gsyDexAddress, challenge, didSignature, substrateSignature });

            // NO FALLBACK. There used to be a branch here that caught the 401 from a
            // fabricated Substrate signature, hand-signed a credential with
            // `JSON.stringify(payload, Object.keys(payload).sort())` and inserted it
            // straight into Mongo. That hid plan §0.5 bug (B) twice over: it never
            // exercised the service's issuance path, and it asserted over a payload whose
            // nested claims the sorted replacer had already stripped. Both ends now share
            // `canonicalize()`, so the real route is the only route.
            expect(issueResponse.status).toBe(201);
            savedCredentialId = issueResponse.body.id;
            expect(savedCredentialId).toBeDefined();
            expect(issueResponse.body.credential.credentialSubject.accountLink.gsyDexAddress)
                .toEqual(gsyDexAddress);
            expect(issueResponse.body.credential.proof.jws).toBeDefined();
        } catch (error: any) {
            console.error('Credential Issuance Error:', error);
            if (issueResponse) {
                 console.error('Issuance Response Status:', issueResponse.status);
                 console.error('Issuance Response Body:', issueResponse.body);
            }
            throw error;
        }

        const issuerTxCountAfter = await provider.getTransactionCount(issuerAddress);
        expect(issuerTxCountAfter).toEqual(issuerTxCountBefore);
    });

    it('should get credentials for the authenticated user', async () => {
        expect(didSuccessfullyRegistered).toBe(true);
        expect(testToken).toBeDefined();
        expect(savedCredentialId).toBeDefined();
        try {
            const response = await request(app.getHttpServer())
            .get(`/credentials/did/${testDid}`)
            .set('Authorization', `Bearer ${testToken}`)
            .expect(200);

            expect(Array.isArray(response.body)).toBe(true);
            expect(response.body.length).toBeGreaterThan(0);
            const foundCredential = response.body.find(c => c.id === savedCredentialId);
            expect(foundCredential).toBeDefined();
            expect(foundCredential.did).toBe(testDid);
            expect(foundCredential.credential.credentialSubject.accountLink.gsyDexAddress).toEqual(gsyDexAddress);
        } catch (error: any) {
            console.error('Get Credentials Error:', error.response?.body || error.message);
            throw error;
        }
    });

    it('should verify and revoke a credential', async () => {
       expect(didSuccessfullyRegistered).toBe(true);
       expect(savedCredentialId).toBeDefined();
       expect(testToken).toBeDefined();

       const credentialRecord = await credentialModel.findOne({ id: savedCredentialId }).lean().exec();
       expect(credentialRecord).toBeDefined();
       expect(credentialRecord.credential).toBeDefined();

       const issuerTxCountBeforeRevoke = await provider.getTransactionCount(issuerAddress);

       let verifyResponse;
       try {
           verifyResponse = await request(app.getHttpServer())
               .post('/credentials/verify')
               .send({ credential: credentialRecord.credential });

           expect(verifyResponse.status).toBe(200);
           expect(verifyResponse.body).toBeDefined();
           expect(verifyResponse.body.valid).toBe(true);
           expect(verifyResponse.body.did).toBe(testDid);
           expect(verifyResponse.body.gsyDexAddress).toBe(gsyDexAddress);

       } catch (error: any) {
           console.error('Credential verification error:', error);
            if (verifyResponse) {
              console.error('Verification Response Status:', verifyResponse.status);
              console.error('Verification Response Body:', verifyResponse.body);
           }
           console.error("Credential that failed verification:", JSON.stringify(credentialRecord.credential, null, 2));
           throw error;
       }

       let revokeResponse;
       try {
           revokeResponse = await request(app.getHttpServer())
               .delete(`/credentials/${savedCredentialId}`)
               .set('Authorization', `Bearer ${testToken}`)
               .expect(200);

           expect(revokeResponse.body).toHaveProperty('success', true);

           const revokedRecord = await credentialModel.findOne({ id: savedCredentialId }).exec();
           expect(revokedRecord?.status).toEqual('revoked');

       } catch (error: any) {
          console.error('Credential revocation error:', error);
          if (revokeResponse) {
           console.error('Revocation Response Status:', revokeResponse.status);
           console.error('Revocation Response Body:', revokeResponse.body);
          }
          throw error;
       }

       const issuerTxCountAfterRevoke = await provider.getTransactionCount(issuerAddress);
       expect(issuerTxCountAfterRevoke).toEqual(issuerTxCountBeforeRevoke);
   });
  });
});