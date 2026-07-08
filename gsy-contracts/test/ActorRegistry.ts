import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { bytes16Id, deployUpgradeableContract } from "./utils";

describe("ActorRegistry", function () {
  async function deployActorRegistryFixture() {
    const [admin, user, delegate] = await ethers.getSigners();
    const actorRegistry = await deployUpgradeableContract("ActorRegistry", [
      admin.address,
    ]);

    const ACTOR_REGISTRAR_ROLE = await actorRegistry.ACTOR_REGISTRAR_ROLE();
    const actorId = bytes16Id("actor:user");
    await actorRegistry.registerActor(actorId, user.address);

    return {
      actorRegistry,
      admin,
      user,
      delegate,
      ACTOR_REGISTRAR_ROLE,
      actorId,
    };
  }

  it("Should restrict actor wallet registration to the registrar role", async function () {
    const { actorRegistry, user, actorId, ACTOR_REGISTRAR_ROLE } =
      await loadFixture(deployActorRegistryFixture);

    await expect(
      actorRegistry.connect(user).registerActor(actorId, user.address),
    )
      .to.be.revertedWithCustomError(
        actorRegistry,
        "AccessControlUnauthorizedAccount",
      )
      .withArgs(user.address, ACTOR_REGISTRAR_ROLE);
  });

  it("Should allow the registrar to revoke an actor wallet", async function () {
    const { actorRegistry, user, actorId } = await loadFixture(
      deployActorRegistryFixture,
    );

    await expect(actorRegistry.setActorWallet(actorId, user.address, false))
      .to.emit(actorRegistry, "ActorWalletUpdated")
      .withArgs(actorId, user.address, false);

    expect(await actorRegistry.isAuthorized(actorId, user.address)).to.be.false;
  });

  it("Should manage proxy settings", async function () {
    const { actorRegistry, user, delegate, actorId } = await loadFixture(
      deployActorRegistryFixture,
    );

    await expect(
      actorRegistry.connect(user).setProxy(actorId, delegate.address, true),
    )
      .to.emit(actorRegistry, "ProxyUpdated")
      .withArgs(actorId, delegate.address, true);

    expect(await actorRegistry.isProxy(actorId, delegate.address)).to.be.true;
    expect(await actorRegistry.isAuthorized(actorId, delegate.address)).to.be
      .true;
  });
});
