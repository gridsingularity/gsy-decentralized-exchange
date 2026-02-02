import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";

describe("GsyVault", function () {
  async function deployVaultFixture() {
    const [admin, user, settlement, delegate] = await ethers.getSigners();
    const GsyVault = await ethers.getContractFactory("GsyVault");
    const vault = await GsyVault.deploy();

    const SETTLEMENT_ROLE = await vault.SETTLEMENT_ROLE();
    await vault.grantRole(SETTLEMENT_ROLE, settlement.address);

    return { vault, admin, user, settlement, delegate, SETTLEMENT_ROLE };
  }

  it("Should accept deposits", async function () {
    const { vault, user } = await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");

    await expect(vault.connect(user).deposit({ value: amount }))
      .to.emit(vault, "Deposited")
      .withArgs(user.address, amount);

    expect(await vault.balances(user.address)).to.equal(amount);
  });

  it("Should allow withdrawals if balance is sufficient", async function () {
    const { vault, user } = await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");
    await vault.connect(user).deposit({ value: amount });

    await expect(vault.connect(user).withdraw(amount)).to.changeEtherBalances(
      [vault, user],
      [-amount, amount],
    );
  });

  it("Should fail withdrawal if insufficient balance", async function () {
    const { vault, user } = await loadFixture(deployVaultFixture);
    await expect(
      vault.connect(user).withdraw(100),
    ).to.be.revertedWithCustomError(vault, "InsufficientBalance");
  });

  it("Should manage proxy settings", async function () {
    const { vault, user, delegate } = await loadFixture(deployVaultFixture);

    await expect(vault.connect(user).setProxy(delegate.address, true))
      .to.emit(vault, "ProxyUpdated")
      .withArgs(user.address, delegate.address, true);

    expect(await vault.isProxy(user.address, delegate.address)).to.be.true;
  });

  it("Should allow settlement transfers only by authorized role", async function () {
    const { vault, user, delegate, settlement } =
      await loadFixture(deployVaultFixture);
    const amount = ethers.parseEther("1.0");
    await vault.connect(user).deposit({ value: amount });

    // Unauthorized attempt
    await expect(
      vault
        .connect(delegate)
        .transferBySettlement(user.address, delegate.address, amount),
    ).to.be.revertedWithCustomError(vault, "AccessControlUnauthorizedAccount");

    // Authorized attempt
    await expect(
      vault
        .connect(settlement)
        .transferBySettlement(user.address, delegate.address, amount),
    )
      .to.emit(vault, "TransferredBySettlement")
      .withArgs(user.address, delegate.address, amount);

    expect(await vault.balances(user.address)).to.equal(0);
    expect(await vault.balances(delegate.address)).to.equal(amount);
  });
});
