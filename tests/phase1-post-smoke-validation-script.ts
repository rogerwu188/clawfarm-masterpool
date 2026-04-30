import { expect } from "chai";
import { getAssociatedTokenAddressSync } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { hashRequestNonce } from "../scripts/phase1/compact-receipt";

import {
  advanceReportStatus,
  computeLinearReleasableAmount,
  computeFinalizeWindow,
  deriveChallengeCaseTargets,
  derivePostSmokeFinalizeTargets,
  normalizeChallengeCases,
  normalizePostSmokeValidationConfig,
  parsePostSmokeValidationArgs,
  validateMainnetFaucetDisabled,
} from "../scripts/phase1/post-smoke-validation";

describe("post-smoke-validation parser", () => {
  it("requires both deployment and config paths", () => {
    expect(() =>
      parsePostSmokeValidationArgs([
        "--deployment",
        "deployments/devnet-phase1.json",
      ])
    ).to.throw("config path is required");
  });

  it("accepts an optional output path", () => {
    const args = parsePostSmokeValidationArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--config",
      "configs/post-smoke.devnet.json",
      "--out",
      "reports/post-smoke.json",
    ]);

    expect(args.deployment).to.equal("deployments/devnet-phase1.json");
    expect(args.config).to.equal("configs/post-smoke.devnet.json");
    expect(args.out).to.equal("reports/post-smoke.json");
  });
});

describe("post-smoke-validation config", () => {
  it("requires admin keypair, receipt, and settlement addresses", () => {
    const config = normalizePostSmokeValidationConfig({
      adminKeypair: "/tmp/admin.json",
      payerKeypair: "/tmp/payer.json",
      providerWalletKeypair: "/tmp/provider.json",
      receiptPda: "GAHA6Kn1iYFFaYkFAdatnGdit93RVqJCTW9Sdwn7vNUV",
      settlementPda: "C47J7rfZRsfYS2thTLD5kHTtaxMXmhD29Zr24MFEzqWf",
    });

    expect(config.adminKeypair).to.equal("/tmp/admin.json");
    expect(config.payerKeypair).to.equal("/tmp/payer.json");
    expect(config.providerWalletKeypair).to.equal("/tmp/provider.json");
    expect(config.receiptPda).to.equal(
      "GAHA6Kn1iYFFaYkFAdatnGdit93RVqJCTW9Sdwn7vNUV"
    );
    expect(config.settlementPda).to.equal(
      "C47J7rfZRsfYS2thTLD5kHTtaxMXmhD29Zr24MFEzqWf"
    );
  });

  it("rejects invalid public keys", () => {
    expect(() =>
      normalizePostSmokeValidationConfig({
        adminKeypair: "/tmp/admin.json",
        receiptPda: "not-a-pubkey",
        settlementPda: "C47J7rfZRsfYS2thTLD5kHTtaxMXmhD29Zr24MFEzqWf",
      })
    ).to.throw("receiptPda must be a valid public key");
  });

  it("normalizes optional challenge cases with inherited receipt defaults", () => {
    const cases = normalizeChallengeCases(
      {
        rejected: {
          requestNonce: "ps-rejected-001",
        },
        timeoutReject: {
          requestNonce: "ps-timeout-001",
          proofId: "timeout-proof",
        },
      },
      {
        providerCode: "u",
        proofId: "base-proof",
        model: "m",
        receiptChargeUiAmount: "10",
        promptTokens: 123,
        completionTokens: 456,
      }
    );

    expect(cases?.rejected?.requestNonce).to.equal("ps-rejected-001");
    expect(cases?.rejected?.providerCode).to.equal("u");
    expect(cases?.rejected?.proofId).to.equal("base-proof");
    expect(cases?.rejected?.receiptChargeBaseUnits).to.equal(10_000_000n);
    expect(cases?.timeoutReject?.proofId).to.equal("timeout-proof");
    expect(cases?.timeoutReject?.completionTokens).to.equal(456);
  });

  it("requires receipt defaults when challenge cases are configured", () => {
    expect(() =>
      normalizeChallengeCases(
        {
          accepted: {
            requestNonce: "ps-accepted-001",
          },
        },
        {
          providerCode: undefined,
          proofId: "base-proof",
          model: "m",
          receiptChargeUiAmount: "10",
          promptTokens: 123,
          completionTokens: 456,
        }
      )
    ).to.throw("providerCode is required when challenge cases are enabled");
  });
});

describe("post-smoke finalize window", () => {
  it("reports the earliest finalize instant and remaining wait time", () => {
    const report = computeFinalizeWindow({
      challengeDeadlineUnixSeconds: 1_700_000_000,
      nowUnixSeconds: 1_699_999_970,
    });

    expect(report.canFinalizeNow).to.equal(false);
    expect(report.secondsUntilFinalizable).to.equal(31);
    expect(report.earliestFinalizeAtUnixSeconds).to.equal(1_700_000_001);
    expect(report.earliestFinalizeAtIso).to.equal(
      "2023-11-14T22:13:21.000Z"
    );
  });

  it("marks finalize as ready only after the deadline has passed", () => {
    const report = computeFinalizeWindow({
      challengeDeadlineUnixSeconds: 1_700_000_000,
      nowUnixSeconds: 1_700_000_001,
    });

    expect(report.canFinalizeNow).to.equal(true);
    expect(report.secondsUntilFinalizable).to.equal(0);
  });
});

describe("post-smoke reward release math", () => {
  it("computes the vested-but-unreleased amount", () => {
    const releasable = computeLinearReleasableAmount({
      totalLocked: 3_000_000,
      releasedSoFar: 0,
      lockStartUnixSeconds: 1_700_000_000,
      nowUnixSeconds: 1_700_000_000 + 6,
      lockDays: 180,
    });

    expect(releasable).to.equal(1);
  });

  it("caps elapsed time at the full lock duration", () => {
    const releasable = computeLinearReleasableAmount({
      totalLocked: 3_000_000,
      releasedSoFar: 200_000,
      lockStartUnixSeconds: 1_700_000_000,
      nowUnixSeconds: 1_700_000_000 + 200 * 86_400,
      lockDays: 180,
    });

    expect(releasable).to.equal(2_800_000);
  });
});

describe("post-smoke report status", () => {
  it("prioritizes failed over waiting and ok", () => {
    expect(advanceReportStatus("ok", "waiting")).to.equal("waiting");
    expect(advanceReportStatus("waiting", "ok")).to.equal("waiting");
    expect(advanceReportStatus("waiting", "failed")).to.equal("failed");
  });
});

describe("mainnet faucet preflight", () => {
  it("passes when faucet config is absent", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", null)).not.to.throw();
  });

  it("passes on devnet even when faucet is enabled", () => {
    expect(() => validateMainnetFaucetDisabled("devnet", { enabled: true })).not.to.throw();
  });

  it("passes on mainnet when faucet is disabled", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", { enabled: false })).not.to.throw();
  });

  it("fails on mainnet when faucet is enabled", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", { enabled: true })).to.throw(
      "mainnet faucet must not be enabled"
    );
  });
});

describe("post-smoke finalize target derivation", () => {
  it("derives provider, reward, user reward, and provider usdc destination", () => {
    const masterpoolProgramId = new PublicKey(
      "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux"
    );
    const providerWallet = new PublicKey(
      "GBCFD9ty31Fu57fwqqdsR32CzRaoZmWjR9x43G3ajMZA"
    );
    const payerUser = new PublicKey(
      "6QWeT6FpJrm8AF1btu6WH2k2Xhq6t5vbheKVfQavmeoZ"
    );
    const usdcMint = new PublicKey(
      "D3vhDe6mtdAgj2t8pu6XnaFXDPdiMDTALTSCZbizfm9P"
    );

    const derived = derivePostSmokeFinalizeTargets({
      masterpoolProgramId,
      providerWallet,
      payerUser,
      usdcMint,
    });

    expect(derived.providerAccountPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("provider"), providerWallet.toBuffer()],
        masterpoolProgramId
      )[0].toBase58()
    );
    expect(derived.providerRewardPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("provider_reward"), providerWallet.toBuffer()],
        masterpoolProgramId
      )[0].toBase58()
    );
    expect(derived.userRewardPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("user_reward"), payerUser.toBuffer()],
        masterpoolProgramId
      )[0].toBase58()
    );
    expect(derived.providerDestinationUsdc.toBase58()).to.equal(
      getAssociatedTokenAddressSync(usdcMint, providerWallet).toBase58()
    );
  });
});

describe("post-smoke challenge target derivation", () => {
  it("derives receipt, settlement, challenge, and bond-record PDAs from request nonce", () => {
    const attestationProgramId = new PublicKey(
      "52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2"
    );
    const masterpoolProgramId = new PublicKey(
      "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux"
    );

    const derived = deriveChallengeCaseTargets({
      attestationProgramId,
      masterpoolProgramId,
      requestNonce: "ps-timeout-001",
    });

    expect(derived.receiptPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("receipt"), hashRequestNonce("ps-timeout-001")],
        attestationProgramId
      )[0].toBase58()
    );
    expect(derived.challengePda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("challenge"), derived.receiptPda.toBuffer()],
        attestationProgramId
      )[0].toBase58()
    );
    expect(derived.settlementPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("receipt_settlement"), derived.receiptPda.toBuffer()],
        masterpoolProgramId
      )[0].toBase58()
    );
    expect(derived.challengeBondRecordPda.toBase58()).to.equal(
      PublicKey.findProgramAddressSync(
        [Buffer.from("challenge_bond_record"), derived.challengePda.toBuffer()],
        masterpoolProgramId
      )[0].toBase58()
    );
  });
});
