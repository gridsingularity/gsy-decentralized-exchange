import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";

function remoteAccounts(): string[] {
  const privateKey = process.env.DEPLOYER_PRIVATE_KEY;
  return privateKey && privateKey.trim().length > 0 ? [privateKey] : [];
}

const config: HardhatUserConfig = {
  solidity: {
    version: "0.8.22",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200,
      },
      evmVersion: "paris",
    },
  },
  networks: {
    hardhat: {
      chainId: 1337,
    },
    anvil: {
      url: process.env.ANVIL_RPC_URL ?? "http://127.0.0.1:8545",
      chainId: Number(process.env.ANVIL_CHAIN_ID ?? 31337),
    },
    ewc: {
      url: process.env.EWC_RPC_URL ?? "https://rpc.energyweb.org",
      chainId: 246,
      accounts: remoteAccounts(),
    },
    volta: {
      url: process.env.VOLTA_RPC_URL ?? "https://volta-rpc.energyweb.org",
      chainId: 73799,
      accounts: remoteAccounts(),
    },
  },
};

export default config;
