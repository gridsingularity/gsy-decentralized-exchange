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
});