import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";

const config: HardhatUserConfig = {
  solidity: {
    version: "0.8.20",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200,
      },
    },
  },
  networks: {
    hardhat: {
      chainId: 1337,
    },
    anvil: {
      url: process.env.ANVIL_RPC_URL ?? "http://127.0.0.1:8545",
      chainId: 31337,
    },
    // Future config for Energy Web Chain (Volta)
    volta: {
      url: "https://volta-rpc.energyweb.org",
      chainId: 73799,
    },
  },
};

export default config;
