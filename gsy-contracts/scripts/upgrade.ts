import { ethers } from "hardhat";

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value || value.trim().length === 0) {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return value;
}

async function main() {
  const contractName = requiredEnv("UPGRADE_CONTRACT_NAME");
  const proxyAddress = requiredEnv("UPGRADE_PROXY_ADDRESS");
  const proxyAdminAddress = requiredEnv("UPGRADE_PROXY_ADMIN_ADDRESS");
  const upgradeCallData = process.env.UPGRADE_CALL_DATA ?? "0x";

  const proxyAdminOwner = process.env.PROXY_ADMIN_PRIVATE_KEY
    ? new ethers.Wallet(process.env.PROXY_ADMIN_PRIVATE_KEY, ethers.provider)
    : (await ethers.getSigners())[0];

  const factory = await ethers.getContractFactory(contractName);
  const implementation = await factory.deploy();
  await implementation.waitForDeployment();
  const implementationAddress = await implementation.getAddress();

  const proxyAdmin = await ethers.getContractAt(
    "ProxyAdmin",
    proxyAdminAddress,
    proxyAdminOwner,
  );

  await (
    await proxyAdmin.upgradeAndCall(
      proxyAddress,
      implementationAddress,
      upgradeCallData,
    )
  ).wait();

  console.log("Contract upgraded:");
  console.log(`  contractName        ${contractName}`);
  console.log(`  proxy               ${proxyAddress}`);
  console.log(`  implementation      ${implementationAddress}`);
  console.log(`  proxyAdmin          ${proxyAdminAddress}`);
  console.log(`  proxyAdminOwner     ${await proxyAdminOwner.getAddress()}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
