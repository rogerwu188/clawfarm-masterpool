import { expect } from "chai";
import { PublicKey } from "@solana/web3.js";

import {
  DEFAULT_FAUCET_LIMITS,
  parseFaucetConfigureArgs,
} from "../scripts/phase1/faucet-configure";
import { parseFaucetFundArgs } from "../scripts/phase1/faucet-fund";
import { buildFaucetStatusReport } from "../scripts/phase1/faucet-status";
import { parseFaucetStatusArgs } from "../scripts/phase1/faucet-status";
import { buildFaucetClaimReport, parseFaucetClaimArgs } from "../scripts/phase1/faucet-claim";

describe("faucet-configure parser", () => {
  it("requires deployment and admin keypair", () => {
    expect(() => parseFaucetConfigureArgs(["--deployment", "deployments/devnet-phase1.json"])).to.throw(
      "admin keypair path is required"
    );
  });

  it("uses six-decimal default faucet limits", () => {
    expect(DEFAULT_FAUCET_LIMITS.maxClawPerClaim.toString()).to.equal("10000000");
    expect(DEFAULT_FAUCET_LIMITS.maxUsdcPerClaim.toString()).to.equal("10000000");
    expect(DEFAULT_FAUCET_LIMITS.maxClawGlobalPerDay.toString()).to.equal("50000000000");
    expect(DEFAULT_FAUCET_LIMITS.maxUsdcGlobalPerDay.toString()).to.equal("50000000000");
  });

  it("parses enable mode", () => {
    const args = parseFaucetConfigureArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--admin-keypair",
      "/tmp/admin.json",
      "--enable",
    ]);
    expect(args.enable).to.equal(true);
    expect(args.disable).to.equal(false);
  });
});

describe("faucet-fund parser", () => {
  it("requires a supported token", () => {
    expect(() => parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--funding-keypair",
      "/tmp/fund.json",
      "--token",
      "btc",
      "--amount",
      "1",
    ])).to.throw("token must be claw or usdc");
  });

  it("parses ui amount into base units", () => {
    const args = parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--funding-keypair",
      "/tmp/fund.json",
      "--token",
      "usdc",
      "--amount",
      "12.5",
    ]);
    expect(args.amountBaseUnits.toString()).to.equal("12500000");
  });

  it("requires admin keypair for CLAW reward-vault funding", () => {
    expect(() => parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--token",
      "claw",
      "--amount",
      "1",
    ])).to.throw("admin keypair path is required for CLAW funding");
  });

  it("accepts admin keypair for CLAW reward-vault funding", () => {
    const args = parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--admin-keypair",
      "/tmp/admin.json",
      "--token",
      "claw",
      "--amount",
      "12.5",
    ]);
    expect(args.adminKeypair).to.equal("/tmp/admin.json");
    expect(args.amountBaseUnits.toString()).to.equal("12500000");
  });
});


describe("faucet-claim parser", () => {
  const recipient = "11111111111111111111111111111111";

  it("requires deployment, recipient public key, and fee payer keypair", () => {
    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--claw-amount",
      "1",
    ])).to.throw("user public key is required");

    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--claw-amount",
      "1",
    ])).to.throw("fee payer keypair path is required");
  });

  it("parses recipient public key, fee payer keypair, and ui claim amounts", () => {
    const args = parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--fee-payer-keypair",
      "/tmp/server-fee-payer.json",
      "--claw-amount",
      "1.5",
      "--usdc-amount",
      "2.25",
    ]);

    expect(args.userPublicKey.toBase58()).to.equal(recipient);
    expect(args.feePayerKeypair).to.equal("/tmp/server-fee-payer.json");
    expect(args.clawAmountBaseUnits.toString()).to.equal("1500000");
    expect(args.usdcAmountBaseUnits.toString()).to.equal("2250000");
  });

  it("requires at least one positive claim amount", () => {
    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--fee-payer-keypair",
      "/tmp/server-fee-payer.json",
    ])).to.throw("at least one claim amount is required");
  });

  it("reports recipient, fee payer, source authority, and wallet daily remaining quota", () => {
    const report = buildFaucetClaimReport({
      signature: "sig",
      userPublicKey: "recipientWallet",
      feePayer: "serverFeePayer",
      sourceAuthority: "poolAuthorityPda",
      userFaucetState: "state",
      userClawToken: "clawAta",
      userUsdcToken: "usdcAta",
      clawAmountBaseUnits: BigInt("10000000"),
      usdcAmountBaseUnits: BigInt("5000000"),
      faucetConfig: {
        maxClawPerWalletPerDay: { toString: () => "50000000" },
        maxUsdcPerWalletPerDay: { toString: () => "50000000" },
      },
      userState: {
        currentDayIndex: { toString: () => "20567" },
        clawClaimedToday: { toString: () => "10000000" },
        usdcClaimedToday: { toString: () => "5000000" },
      },
    });

    expect(report.recipient).to.equal("recipientWallet");
    expect(report.userPublicKey).to.equal("recipientWallet");
    expect(report.feePayer).to.equal("serverFeePayer");
    expect(report.sourceAuthority).to.equal("poolAuthorityPda");
    expect(report.walletDailyQuota.claw.remaining).to.equal("40000000");
    expect(report.walletDailyQuota.usdc.remaining).to.equal("45000000");
  });
});

describe("faucet-status parser", () => {
  it("requires deployment", () => {
    expect(() => parseFaucetStatusArgs([])).to.throw("deployment path is required");
  });

  it("includes reward vault and faucet claw vault balances in status reports", () => {
    const report = buildFaucetStatusReport({
      currentDayIndex: 123,
      pdas: {
        faucetConfig: new PublicKey("11111111111111111111111111111111"),
        faucetGlobal: new PublicKey("11111111111111111111111111111111"),
        faucetClawVault: new PublicKey("11111111111111111111111111111111"),
        faucetUsdcVault: new PublicKey("11111111111111111111111111111111"),
      },
      faucetConfig: { enabled: true },
      faucetGlobalState: { clawClaimedToday: "0", usdcClaimedToday: "0" },
      rewardVault: {
        mint: new PublicKey("11111111111111111111111111111111"),
        owner: new PublicKey("11111111111111111111111111111111"),
        amount: BigInt("999000000000000"),
      },
      clawVault: {
        mint: new PublicKey("11111111111111111111111111111111"),
        owner: new PublicKey("11111111111111111111111111111111"),
        amount: BigInt("1000000000000"),
      },
      usdcVault: {
        mint: new PublicKey("11111111111111111111111111111111"),
        owner: new PublicKey("11111111111111111111111111111111"),
        amount: BigInt("500000000"),
      },
    });

    expect(report.vaults.rewardClaw.amount).to.equal("999000000000000");
    expect(report.vaults.faucetClaw.amount).to.equal("1000000000000");
    expect(report.vaults.faucetUsdc.amount).to.equal("500000000");
  });
});
