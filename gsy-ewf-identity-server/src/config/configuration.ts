export default () => ({
  port: parseInt(process.env.PORT, 10) || 3000,
  nodeEnv: process.env.NODE_ENV || 'development',
  
  mongodb: {
    uri: process.env.MONGODB_URI,
  },
  
  ewc: {
    rpcUrl: process.env.EWC_RPC_URL,
    didRegistryAddress: process.env.DID_REGISTRY_ADDRESS,
    issuerPrivateKey: process.env.ISSUER_PRIVATE_KEY,
    issuerPublicKey: process.env.ISSUER_PUBLIC_KEY,
  },

  substrate: {
    wsUrl: process.env.SUBSTRATE_WS_URL,
  },

  identity: {
    // Machine-to-machine API key (ApiKeyGuard). API_KEY is deliberately shared with
    // gsy-offchain-storage for now (gsy-offchain-storage/src/configuration.rs:22-23);
    // IDENTITY_API_KEY is read first so the keys can be split later without a code change.
    // Empty/unset => ApiKeyGuard throws at construction and the service refuses to start.
    apiKey: process.env.IDENTITY_API_KEY ?? process.env.API_KEY ?? '',
  },
  
  jwt: {
    secret: process.env.JWT_SECRET || 'supersecret',
    expiresIn: process.env.JWT_EXPIRES_IN || '24h',
  },
});