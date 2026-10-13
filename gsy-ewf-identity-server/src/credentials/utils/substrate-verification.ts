import { cryptoWaitReady, decodeAddress, signatureVerify } from '@polkadot/util-crypto';
import { u8aToHex, stringToU8a } from '@polkadot/util';
import { Logger } from '@nestjs/common';

const logger = new Logger('SubstrateVerification');

// Initialize crypto when module is loaded
let cryptoInitialized = false;
const initCrypto = async () => {
  if (!cryptoInitialized) {
    await cryptoWaitReady();
    cryptoInitialized = true;
    logger.log('Substrate crypto initialized');
  }
};

// Initialize crypto right away
initCrypto().catch(error => {
  logger.error(`Failed to initialize Substrate crypto: ${error.message}`);
});

/**
 * Verify a signature created by a Substrate account
 * @param message The message that was signed
 * @param signature The signature as a hex string (with or without 0x prefix)
 * @param address The Substrate address that supposedly signed the message
 * @returns boolean indicating if the signature is valid
 */
export async function verifySubstrateSignature(
  message: string,
  signature: string,
  address: string,
): Promise<boolean> {
  try {
    // Make sure crypto is initialized
    if (!cryptoInitialized) {
      await cryptoWaitReady();
      cryptoInitialized = true;
    }

    // Normalize the signature (ensure it has 0x prefix)
    const normalizedSignature = signature.startsWith('0x') ? signature : `0x${signature}`;
    
    // Get public key from address
    const publicKey = decodeAddress(address);
    const hexPublicKey = u8aToHex(publicKey);

    // Convert the message to a Uint8Array (if needed)
    const messageU8a = typeof message === 'string' ? stringToU8a(message) : message;

    // Verify the signature
    const { isValid, crypto } = signatureVerify(messageU8a, normalizedSignature, hexPublicKey);

    logger.debug(`Signature verification result: valid=${isValid}, crypto=${crypto}, address=${address}`);

    return isValid;
  } catch (error) {
    logger.error(`Failed to verify Substrate signature: ${error.message}`);
    return false;
  }
}

/**
 * Format a message for Substrate signing
 * This prefixes the message with <Bytes> and </Bytes> 
 * to make it compatible with how wallets like Polkadot.js extension sign messages
 * @param message The message to format
 * @returns Formatted message
 */
export function formatSubstrateSigningMessage(message: string): string {
  return `<Bytes>${message}</Bytes>`;
}