import { verifySubstrateSignature, formatSubstrateSigningMessage } from '../src/credentials/utils/substrate-verification';

// Mock the @polkadot/util-crypto and @polkadot/util packages
jest.mock('@polkadot/util-crypto', () => ({
  signatureVerify: jest.fn((message, signature, hexPublicKey) => {
    // Return valid for specific test cases
    if (signature === '0x01234567890abcdef' && hexPublicKey.includes('5G9VQ59Hj4K')) {
      return { isValid: true, crypto: 'sr25519' };
    }
    
    // Invalid for other cases
    return { isValid: false, crypto: 'sr25519' };
  }),
  decodeAddress: jest.fn(() => new Uint8Array([1, 2, 3, 4])),
  cryptoWaitReady: jest.fn().mockResolvedValue(true),
}));

jest.mock('@polkadot/util', () => ({
  hexToU8a: jest.fn(hex => new Uint8Array([1, 2, 3])),
  u8aToHex: jest.fn(arr => '0x5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN'),
  stringToU8a: jest.fn(str => new Uint8Array([...str].map(c => c.charCodeAt(0)))),
}));

describe('Substrate Verification Utilities', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('formatSubstrateSigningMessage', () => {
    it('should format a message for Substrate signing', () => {
      const message = 'Test message';
      const formatted = formatSubstrateSigningMessage(message);
      expect(formatted).toBe('<Bytes>Test message</Bytes>');
    });
  });

  describe('verifySubstrateSignature', () => {
    it('should return true for valid signature', async () => {
      const message = '<Bytes>Test message</Bytes>';
      const signature = '0x01234567890abcdef';
      const address = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      
      const result = await verifySubstrateSignature(message, signature, address);
      
      expect(result).toBe(true);
    });

    it('should return false for invalid signature', async () => {
      const message = '<Bytes>Test message</Bytes>';
      const signature = '0xdeadbeef';
      const address = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      
      const result = await verifySubstrateSignature(message, signature, address);
      
      expect(result).toBe(false);
    });

    it('should handle signatures without 0x prefix', async () => {
      const message = '<Bytes>Test message</Bytes>';
      const signature = '01234567890abcdef'; // No 0x prefix
      const address = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      
      const result = await verifySubstrateSignature(message, signature, address);
      
      // The function should add the 0x prefix and verify
      expect(result).toBe(true);
    });

    it('should return false when an error occurs', async () => {
      // Setup the mock to throw an error
      const { signatureVerify } = require('@polkadot/util-crypto');
      signatureVerify.mockImplementationOnce(() => {
        throw new Error('Test error');
      });
      
      const message = '<Bytes>Test message</Bytes>';
      const signature = '0x01234567890abcdef';
      const address = '5G9VQ59Hj4Kcq8QgQKM3D1ZxY71zKxgEqj4MBSTS9LM2FPTN';
      
      const result = await verifySubstrateSignature(message, signature, address);
      
      // Should gracefully handle errors and return false
      expect(result).toBe(false);
    });
  });
});