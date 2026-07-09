import { expect } from "chai";
import { ethers } from "hardhat";
import {
  bytes16Id,
  deployUpgradeableContract,
  getProxyAdminAddress,
} from "./utils";

describe("Upgradeable contracts", function () {
  it("Should upgrade ActorRegistry through ProxyAdmin and preserve state", async function () {
    const [admin, user] = await ethers.getSigners();
    const actorRegistry = await deployUpgradeableContract("ActorRegistry", [
      admin.address,
    ]);
    const actorId = bytes16Id("actor:user");

    await actorRegistry.registerActor(actorId, user.address);
    expect(await actorRegistry.isAuthorized(actorId, user.address)).to.be.true;

    const actorRegistryProxyAddress = await actorRegistry.getAddress();
    const proxyAdminAddress = await getProxyAdminAddress(
      actorRegistryProxyAddress,
    );
    const proxyAdmin = await ethers.getContractAt(
      "ProxyAdmin",
      proxyAdminAddress,
      admin,
    );

    const ActorRegistryV2Mock =
      await ethers.getContractFactory("ActorRegistryV2Mock");
    const actorRegistryV2Implementation = await ActorRegistryV2Mock.deploy();
    await actorRegistryV2Implementation.waitForDeployment();

    await proxyAdmin.upgradeAndCall(
      actorRegistryProxyAddress,
      await actorRegistryV2Implementation.getAddress(),
      "0x",
    );

    const upgradedActorRegistry = ActorRegistryV2Mock.attach(
      actorRegistryProxyAddress,
    );
    expect(await upgradedActorRegistry.version()).to.equal(2);
    expect(await upgradedActorRegistry.isAuthorized(actorId, user.address)).to.be
      .true;
  });
});
