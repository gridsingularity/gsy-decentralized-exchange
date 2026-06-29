import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { bytes16Id } from "./utils";

describe("GsyVault", function () {
  async function deployVaultFixture() {
    const [admin, user, settlement, delegate] = await ethers.getSigners();
    const GsyVault = await ethers.getContractFactory("GsyVault");
    const vault = await GsyVault.deploy();

    const SETTLEMENT_ROLE = await vault.SETTLEMENT_ROLE();
    await vault.grantRole(SETTLEMENT_ROLE, settlement.address);

    const ACTOR_REGISTRAR_ROLE = await vault.ACTOR_REGISTRAR_ROLE();
    const actorId = bytes16Id("actor:user");
    const delegateActorId = bytes16Id("actor:delegate");
    await vault.registerActor(actorId, user.address);
    await vault.registerActor(delegateActorId, delegate.address);

    return {
      vault,
      admin,
      user,
      settlement,
      delegate,
      SETTLEMENT_ROLE,
      ACTOR_REGISTRAR_ROLE,
      actorId,
      delegateActorId,
    };
  }

  it("Should restrict actor wallet registration to the registrar role", async function () {
    const { vault, user, actorId, ACTOR_REGISTRAR_ROLE } =
      await loadFixture(deployVaultFixture);

    await expect(
      vault.connect(user).registerActor(actorId, user.address),
    )
      .to.be.revertedWithCustomError(vault, "AccessControlUnauthorizedAccount")
      .withArgs(user.address, ACTOR_REGISTRAR_ROLE);
  });

  it("Should allow the registrar to revoke an actor wallet", async function () {
    const { vault, user, actorId } = await loadFixture(deployVaultFixture);

    await expect(vault.setActorWallet(actorId, user.address, false))
      .to.emit(vault, "ActorWalletUpdated")
      .withArgs(actorId, user.address, false);

    await expect(
      vault.connect(user).deposit(actorId, { value: ethers.parseEther("1.0") }),
    ).to.be.revertedWithCustomError(vault, "UnauthorizedActorWallet");
  });

  it("Should accept deposits for an actor", async function () {
    const { vault, user, actorId } = await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");

    await expect(vault.connect(user).deposit(actorId, { value: amount }))
      .to.emit(vault, "Deposited")
      .withArgs(actorId, user.address, amount);

    expect(await vault.balances(actorId)).to.equal(amount);
  });

  it("Should allow withdrawals if balance is sufficient", async function () {
    const { vault, user, actorId } = await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");
    await vault.connect(user).deposit(actorId, { value: amount });

    await expect(
      vault.connect(user).withdraw(actorId, amount),
    ).to.changeEtherBalances([vault, user], [-amount, amount]);
  });

  it("Should fail withdrawal if insufficient balance", async function () {
    const { vault, user, actorId } = await loadFixture(deployVaultFixture);
    await expect(
      vault.connect(user).withdraw(actorId, 100),
    ).to.be.revertedWithCustomError(vault, "InsufficientBalance");
  });

  it("Should manage proxy settings", async function () {
    const { vault, user, delegate, actorId } =
      await loadFixture(deployVaultFixture);

    await expect(vault.connect(user).setProxy(actorId, delegate.address, true))
      .to.emit(vault, "ProxyUpdated")
      .withArgs(actorId, delegate.address, true);

    expect(await vault.isProxy(actorId, delegate.address)).to.be.true;
  });

  it("Should allow settlement transfers only by authorized role", async function () {
    const { vault, user, delegate, settlement, actorId, delegateActorId } =
      await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");
    await vault.connect(user).deposit(actorId, { value: amount });

    await expect(
      vault
        .connect(delegate)
        .transferBySettlement(actorId, delegateActorId, amount),
    ).to.be.revertedWithCustomError(vault, "AccessControlUnauthorizedAccount");

    await expect(
      vault
        .connect(settlement)
        .transferBySettlement(actorId, delegateActorId, amount),
    )
      .to.emit(vault, "TransferredBySettlement")
      .withArgs(actorId, delegateActorId, amount);

    expect(await vault.balances(actorId)).to.equal(0);
    expect(await vault.balances(delegateActorId)).to.equal(amount);
  });
});
