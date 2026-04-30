import * as anchor from "@coral-xyz/anchor";
import { promises as fs, readFileSync } from "fs";
import path from "path";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import {
  Ed25519Program,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";

import {
  DeploymentRecord,
  deriveAttestationConfig,
  deriveMasterpoolPdas,
  loadKeypair,
  toBaseUnits,
} from "./common";
import {
  buildCompactReceiptMetadata,
  buildCompactSubmitArgs,
  findReceiptByHash,
  hashRequestNonce,
  receiptHashHex,
} from "./compact-receipt";

const GATEWAY_ATTESTER_TYPE = 1;
const GATEWAY_ATTESTER_TYPE_MASK = 1 << GATEWAY_ATTESTER_TYPE;
const BPS_SCALE = BigInt(1000);
const DEFAULT_REQUEST_NONCE_PREFIX = "s";
const DEFAULT_PROOF_ID = "r";
const DEFAULT_MODEL = "m";

type ReportStatus = "ok" | "failed";

export interface DevnetSmokeTestArgs {
  deployment: string;
  config: string;
  out?: string;
}

export interface RawSmokeTestConfig {
  adminKeypair?: string;
  providerWalletKeypair?: string;
  providerSignerKeypair?: string;
  payerKeypair?: string;
  negativeProviderWalletKeypair?: string;
  providerCode?: string;
  receiptChargeUiAmount?: string;
  requestNoncePrefix?: string;
  proofId?: string;
  model?: string;
  promptTokens?: number;
  completionTokens?: number;
  attesterTypeMask?: number;
}

export interface SmokeTestConfig {
  adminKeypair: string;
  providerWalletKeypair: string;
  providerSignerKeypair: string;
  payerKeypair: string;
  negativeProviderWalletKeypair: string;
  providerCode: string;
  receiptChargeUiAmount: string;
  receiptChargeBaseUnits: bigint;
  requestNoncePrefix: string;
  proofId: string;
  model: string;
  promptTokens: number;
  completionTokens: number;
  attesterTypeMask: number;
}

interface SmokeTestReport {
  status: ReportStatus;
  startedAt: string;
  finishedAt?: string;
  deployment: string;
  config: string;
  requestNoncePrefix?: string;
  steps: {
    chainConfig?: Record<string, unknown>;
    providerSigner?: Record<string, unknown>;
    providerRegistration?: Record<string, unknown>;
    invalidUsdcMint?: Record<string, unknown>;
    receiptInvalidUsdcMint?: Record<string, unknown>;
    receiptSubmission?: Record<string, unknown>;
  };
  error?: string;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:smoketest:devnet --deployment <path> --config <path> [--out <path>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --config",
    "",
    "Optional flags:",
    "  --out reports/phase1-devnet-smoketest.json",
  ].join("\n");
}

export function parseDevnetSmokeTestArgs(argv: string[]): DevnetSmokeTestArgs {
  const deployment = valueOf(argv, "--deployment");
  const config = valueOf(argv, "--config");
  const out = valueOf(argv, "--out");

  if (!deployment) throw new Error("deployment path is required");
  if (!config) throw new Error("config path is required");

  return {
    deployment,
    config,
    out,
  };
}

export function normalizeSmokeTestConfig(
  raw: RawSmokeTestConfig
): SmokeTestConfig {
  const adminKeypair = requireNonEmptyString(raw.adminKeypair, "adminKeypair");
  const providerWalletKeypair = requireNonEmptyString(
    raw.providerWalletKeypair,
    "providerWalletKeypair"
  );
  const providerSignerKeypair = requireNonEmptyString(
    raw.providerSignerKeypair,
    "providerSignerKeypair"
  );
  const payerKeypair = requireNonEmptyString(raw.payerKeypair, "payerKeypair");
  const negativeProviderWalletKeypair = requireNonEmptyString(
    raw.negativeProviderWalletKeypair,
    "negativeProviderWalletKeypair"
  );
  const providerCode = requireNonEmptyString(raw.providerCode, "providerCode");
  const receiptChargeUiAmount = requireNonEmptyString(
    raw.receiptChargeUiAmount,
    "receiptChargeUiAmount"
  );
  const promptTokens = normalizeNonNegativeInteger(raw.promptTokens, 123, "promptTokens");
  const completionTokens = normalizeNonNegativeInteger(
    raw.completionTokens,
    456,
    "completionTokens"
  );
  const attesterTypeMask = normalizePositiveInteger(
    raw.attesterTypeMask,
    GATEWAY_ATTESTER_TYPE_MASK,
    "attesterTypeMask"
  );

  return {
    adminKeypair,
    providerWalletKeypair,
    providerSignerKeypair,
    payerKeypair,
    negativeProviderWalletKeypair,
    providerCode,
    receiptChargeUiAmount,
    receiptChargeBaseUnits: toBaseUnits(receiptChargeUiAmount, 6),
    requestNoncePrefix: normalizeCompactAlias(
      raw.requestNoncePrefix,
      DEFAULT_REQUEST_NONCE_PREFIX,
      [["smoke", DEFAULT_REQUEST_NONCE_PREFIX]]
    ),
    proofId: normalizeCompactAlias(raw.proofId, DEFAULT_PROOF_ID, [
      ["smoke-proof", DEFAULT_PROOF_ID],
    ]),
    model: normalizeCompactAlias(raw.model, DEFAULT_MODEL, [
      ["smoke-model", DEFAULT_MODEL],
    ]),
    promptTokens,
    completionTokens,
    attesterTypeMask,
  };
}

async function loadSmokeTestConfig(configPath: string): Promise<SmokeTestConfig> {
  const raw = JSON.parse(
    await fs.readFile(configPath, "utf8")
  ) as RawSmokeTestConfig;
  return normalizeSmokeTestConfig(raw);
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseDevnetSmokeTestArgs(argv);
  const report: SmokeTestReport = {
    status: "failed",
    startedAt: new Date().toISOString(),
    deployment: args.deployment,
    config: args.config,
    steps: {},
  };

  try {
    const deployment = JSON.parse(
      await fs.readFile(args.deployment, "utf8")
    ) as DeploymentRecord;
    const smokeConfig = await loadSmokeTestConfig(args.config);
    report.requestNoncePrefix = smokeConfig.requestNoncePrefix;

    logStep("loading keypairs and programs");
    const admin = await loadKeypair(smokeConfig.adminKeypair);
    const providerWallet = await loadKeypair(smokeConfig.providerWalletKeypair);
    const providerSigner = await loadKeypair(smokeConfig.providerSignerKeypair);
    const payer = await loadKeypair(smokeConfig.payerKeypair);
    const negativeProviderWallet = await loadKeypair(
      smokeConfig.negativeProviderWalletKeypair
    );

    if (deployment.adminAuthority !== admin.publicKey.toBase58()) {
      throw new Error("admin keypair does not match deployment adminAuthority");
    }

    const connection = new anchor.web3.Connection(deployment.rpcUrl, "confirmed");
    const wallet = new anchor.Wallet(admin);
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(provider);

    const masterpoolProgramId = new PublicKey(deployment.masterpoolProgramId);
    const attestationProgramId = new PublicKey(deployment.attestationProgramId);
    const clawMint = new PublicKey(deployment.clawMint);
    const testUsdcMint = new PublicKey(deployment.testUsdcMint);

    const masterpoolIdl = JSON.parse(
      readFileSync("target/idl/clawfarm_masterpool.json", "utf8")
    );
    const attestationIdl = JSON.parse(
      readFileSync("target/idl/clawfarm_attestation.json", "utf8")
    );
    masterpoolIdl.address = masterpoolProgramId.toBase58();
    attestationIdl.address = attestationProgramId.toBase58();

    const masterpool = new anchor.Program(masterpoolIdl as anchor.Idl, provider);
    const attestation = new anchor.Program(attestationIdl as anchor.Idl, provider);

    const pdas = deriveMasterpoolPdas(masterpoolProgramId);
    const attestationConfigPda = deriveAttestationConfig(attestationProgramId);
    const providerSignerPda = deriveProviderSignerPda(
      attestationProgramId,
      providerWallet.publicKey,
      providerSigner.publicKey
    );
    const providerAccountPda = deriveProviderAccountPda(
      masterpoolProgramId,
      providerWallet.publicKey
    );
    const providerRewardPda = deriveProviderRewardAccountPda(
      masterpoolProgramId,
      providerWallet.publicKey
    );
    const userRewardPda = deriveUserRewardAccountPda(
      masterpoolProgramId,
      payer.publicKey
    );
    const negativeProviderAccountPda = deriveProviderAccountPda(
      masterpoolProgramId,
      negativeProviderWallet.publicKey
    );
    const negativeProviderRewardPda = deriveProviderRewardAccountPda(
      masterpoolProgramId,
      negativeProviderWallet.publicKey
    );

    report.steps.chainConfig = await verifyChainConfig({
      deployment,
      masterpool,
      attestation,
      masterpoolProgramId,
      attestationProgramId,
      pdas,
      attestationConfigPda,
    });

    const providerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        admin,
        testUsdcMint,
        providerWallet.publicKey
      )
    ).address;
    const payerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        admin,
        testUsdcMint,
        payer.publicKey
      )
    ).address;

    logStep("upserting provider signer");
    const upsertSignature = await attestation.methods
      .upsertProviderSigner(
        providerSigner.publicKey,
        providerWallet.publicKey,
        smokeConfig.attesterTypeMask,
        new anchor.BN(0),
        new anchor.BN(0)
      )
      .accounts({
        authority: admin.publicKey,
        config: attestationConfigPda,
        providerSigner: providerSignerPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();
    const providerSignerState = await (attestation.account as any).providerSigner.fetch(
      providerSignerPda
    );
    report.steps.providerSigner = {
      signature: upsertSignature,
      providerSignerPda: providerSignerPda.toBase58(),
      providerWallet: providerSignerState.providerWallet.toBase58(),
      attesterTypeMask: providerSignerState.attesterTypeMask,
      status: providerSignerState.status,
    };

    logStep("checking provider registration state");
    report.steps.providerRegistration = await ensureProviderRegistered({
      connection,
      masterpool,
      masterpoolConfigPda: pdas.masterpoolConfig,
      providerAccountPda,
      providerRewardPda,
      providerWallet,
      providerUsdcAta,
      providerStakeUsdcVault: pdas.providerStakeUsdcVault,
      testUsdcMint,
    });

    if (await connection.getAccountInfo(negativeProviderAccountPda, "confirmed")) {
      throw new Error(
        "negative provider account already exists; use an unused negativeProviderWalletKeypair"
      );
    }

    logStep("running InvalidUsdcMint negative check");
    const rogueMint = await createMint(connection, admin, admin.publicKey, null, 6);
    const negativeProviderRogueAta = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        admin,
        rogueMint,
        negativeProviderWallet.publicKey
      )
    ).address;
    const invalidUsdcMintError = await expectErrorContaining(
      () =>
        masterpool.methods
          .registerProvider()
          .accounts({
            config: pdas.masterpoolConfig,
            providerAccount: negativeProviderAccountPda,
            providerRewardAccount: negativeProviderRewardPda,
            providerWallet: negativeProviderWallet.publicKey,
            providerStakeUsdcVault: pdas.providerStakeUsdcVault,
            providerUsdcToken: negativeProviderRogueAta,
            usdcMint: rogueMint,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([negativeProviderWallet])
          .rpc(),
      "InvalidUsdcMint"
    );
    report.steps.invalidUsdcMint = {
      rogueMint: rogueMint.toBase58(),
      providerWallet: negativeProviderWallet.publicKey.toBase58(),
      providerUsdcToken: negativeProviderRogueAta.toBase58(),
      matchedError: "InvalidUsdcMint",
      error: invalidUsdcMintError,
    };

    logStep("running receipt InvalidUsdcMint negative check");
    const mismatchRequestNonce = buildRequestNonce(
      smokeConfig.requestNoncePrefix,
      "x"
    );
    const mismatchError = await expectErrorContaining(
      async () => {
        const submitArgs = makeSubmitArgs({
          smokeConfig,
          requestNonce: mismatchRequestNonce,
          providerWallet: providerWallet.publicKey,
          payerUser: payer.publicKey,
          usdcMint: rogueMint,
        });
        await submitReceipt({
          provider,
          attestation,
          providerSignerSecretKey: providerSigner.secretKey,
          submitArgs,
          attestationConfigPda,
          providerSignerPda,
          payer,
          payerUsdcAta,
          masterpoolConfigPda: pdas.masterpoolConfig,
          masterpoolProgramId,
          providerAccountPda,
          providerRewardPda,
          userRewardPda,
          treasuryUsdcVaultPda: pdas.treasuryUsdcVault,
          providerPendingUsdcVaultPda: pdas.providerPendingUsdcVault,
          usdcMint: rogueMint,
        });
      },
      "InvalidUsdcMint"
    );
    report.steps.receiptInvalidUsdcMint = {
      requestNonce: mismatchRequestNonce,
      rogueMint: rogueMint.toBase58(),
      matchedError: "InvalidUsdcMint",
      error: mismatchError,
    };

    logStep("submitting positive smoke receipt");
    const globalConfig = await (masterpool.account as any).globalConfig.fetch(
      pdas.masterpoolConfig
    );
    const payerUsdcBefore = (await getAccount(connection, payerUsdcAta)).amount;
    const treasuryBefore = (await getAccount(connection, pdas.treasuryUsdcVault)).amount;
    const providerPendingBefore = (
      await getAccount(connection, pdas.providerPendingUsdcVault)
    ).amount;
    const providerStateBefore = await (masterpool.account as any).providerAccount.fetch(
      providerAccountPda
    );

    const successRequestNonce = buildRequestNonce(
      smokeConfig.requestNoncePrefix,
      "o"
    );
    const submitArgs = makeSubmitArgs({
      smokeConfig,
      requestNonce: successRequestNonce,
      providerWallet: providerWallet.publicKey,
      payerUser: payer.publicKey,
      usdcMint: testUsdcMint,
    });
    const positive = await submitReceipt({
      provider,
      attestation,
      providerSignerSecretKey: providerSigner.secretKey,
      submitArgs,
      attestationConfigPda,
      providerSignerPda,
      payer,
      payerUsdcAta,
      masterpoolConfigPda: pdas.masterpoolConfig,
      masterpoolProgramId,
      providerAccountPda,
      providerRewardPda,
      userRewardPda,
      treasuryUsdcVaultPda: pdas.treasuryUsdcVault,
      providerPendingUsdcVaultPda: pdas.providerPendingUsdcVault,
      usdcMint: testUsdcMint,
    });

    const receiptState = await (attestation.account as any).receipt.fetch(
      positive.receiptPda
    );
    const settlementState = await (masterpool.account as any).receiptSettlement.fetch(
      positive.settlementPda
    );
    const payerUsdcAfter = (await getAccount(connection, payerUsdcAta)).amount;
    const treasuryAfter = (await getAccount(connection, pdas.treasuryUsdcVault)).amount;
    const providerPendingAfter = (
      await getAccount(connection, pdas.providerPendingUsdcVault)
    ).amount;
    const providerStateAfter = await (masterpool.account as any).providerAccount.fetch(
      providerAccountPda
    );

    const totalPaid = BigInt(smokeConfig.receiptChargeBaseUnits.toString());
    const expectedTreasury = calculateBpsAmount(
      totalPaid,
      BigInt(globalConfig.treasuryUsdcShareBps)
    );
    const expectedProviderPending = totalPaid - expectedTreasury;

    assertBigIntEquals(
      treasuryAfter - treasuryBefore,
      expectedTreasury,
      "treasury vault delta"
    );
    assertBigIntEquals(
      providerPendingAfter - providerPendingBefore,
      expectedProviderPending,
      "provider pending vault delta"
    );
    assertBigIntEquals(
      payerUsdcBefore - payerUsdcAfter,
      totalPaid,
      "payer usdc delta"
    );
    assertBigIntEquals(
      BigInt(providerStateAfter.pendingProviderUsdc.toString()) -
        BigInt(providerStateBefore.pendingProviderUsdc.toString()),
      expectedProviderPending,
      "provider pending usdc delta"
    );
    assertBigIntEquals(
      BigInt(settlementState.usdcTotalPaid.toString()),
      totalPaid,
      "settlement usdc total"
    );
    const receiptLookupPda = await findReceiptByHash(
      connection,
      attestationProgramId,
      Uint8Array.from(submitArgs.compactArgs.receiptHash)
    );
    if (!receiptLookupPda) {
      throw new Error("receipt hash lookup returned no matching receipt account");
    }
    if (!receiptLookupPda.equals(positive.receiptPda)) {
      throw new Error(
        `receipt hash lookup mismatch: ${receiptLookupPda.toBase58()} != ${positive.receiptPda.toBase58()}`
      );
    }

    report.steps.receiptSubmission = {
      signature: positive.signature,
      requestNonce: successRequestNonce,
      receiptPda: positive.receiptPda.toBase58(),
      receiptHashHex: submitArgs.receiptHashHex,
      receiptLookupPda: receiptLookupPda.toBase58(),
      settlementPda: positive.settlementPda.toBase58(),
      payerUsdcBefore: payerUsdcBefore.toString(),
      payerUsdcAfter: payerUsdcAfter.toString(),
      treasuryVaultBefore: treasuryBefore.toString(),
      treasuryVaultAfter: treasuryAfter.toString(),
      providerPendingVaultBefore: providerPendingBefore.toString(),
      providerPendingVaultAfter: providerPendingAfter.toString(),
      expectedTreasuryDelta: expectedTreasury.toString(),
      expectedProviderPendingDelta: expectedProviderPending.toString(),
      receiptStatus: receiptState.status,
      settlementStatus: settlementState.status,
    };

    report.status = "ok";
  } catch (error) {
    report.error = serializeError(error);
  } finally {
    report.finishedAt = new Date().toISOString();
    if (args.out) {
      await writeJsonFile(args.out, report);
      console.error(`wrote smoke report to ${args.out}`);
    }
    console.log(JSON.stringify(report, null, 2));
    if (report.status !== "ok") {
      process.exitCode = 1;
    }
  }
}

async function verifyChainConfig(args: {
  deployment: DeploymentRecord;
  masterpool: anchor.Program<any>;
  attestation: anchor.Program<any>;
  masterpoolProgramId: PublicKey;
  attestationProgramId: PublicKey;
  pdas: ReturnType<typeof deriveMasterpoolPdas>;
  attestationConfigPda: PublicKey;
}): Promise<Record<string, unknown>> {
  const {
    deployment,
    masterpool,
    attestation,
    masterpoolProgramId,
    attestationProgramId,
    pdas,
    attestationConfigPda,
  } = args;

  assertPubkeyEquals(pdas.masterpoolConfig, deployment.masterpoolConfig, "masterpoolConfig");
  assertPubkeyEquals(pdas.poolAuthority, deployment.poolAuthority, "poolAuthority");
  assertPubkeyEquals(pdas.rewardVault, deployment.rewardVault, "rewardVault");
  assertPubkeyEquals(
    pdas.challengeBondVault,
    deployment.challengeBondVault,
    "challengeBondVault"
  );
  assertPubkeyEquals(
    pdas.treasuryUsdcVault,
    deployment.treasuryUsdcVault,
    "treasuryUsdcVault"
  );
  assertPubkeyEquals(
    pdas.providerStakeUsdcVault,
    deployment.providerStakeUsdcVault,
    "providerStakeUsdcVault"
  );
  assertPubkeyEquals(
    pdas.providerPendingUsdcVault,
    deployment.providerPendingUsdcVault,
    "providerPendingUsdcVault"
  );
  assertPubkeyEquals(
    attestationConfigPda,
    deployment.attestationConfig,
    "attestationConfig"
  );

  const globalConfig = await (masterpool.account as any).globalConfig.fetch(
    pdas.masterpoolConfig
  );
  const attestationConfig = await (attestation.account as any).config.fetch(
    attestationConfigPda
  );

  assertPubkeyEquals(globalConfig.clawMint, deployment.clawMint, "globalConfig.clawMint");
  assertPubkeyEquals(globalConfig.usdcMint, deployment.testUsdcMint, "globalConfig.usdcMint");
  assertPubkeyEquals(
    globalConfig.attestationProgram,
    attestationProgramId,
    "globalConfig.attestationProgram"
  );
  assertPubkeyEquals(
    attestationConfig.masterpoolProgram,
    masterpoolProgramId,
    "attestationConfig.masterpoolProgram"
  );

  return {
    masterpoolProgramId: masterpoolProgramId.toBase58(),
    attestationProgramId: attestationProgramId.toBase58(),
    clawMint: globalConfig.clawMint.toBase58(),
    testUsdcMint: globalConfig.usdcMint.toBase58(),
    providerStakeUsdc: globalConfig.providerStakeUsdc.toString(),
    treasuryUsdcShareBps: globalConfig.treasuryUsdcShareBps,
    providerUsdcShareBps: globalConfig.providerUsdcShareBps,
    attestationConfig: attestationConfigPda.toBase58(),
  };
}

async function ensureProviderRegistered(args: {
  connection: anchor.web3.Connection;
  masterpool: anchor.Program<any>;
  masterpoolConfigPda: PublicKey;
  providerAccountPda: PublicKey;
  providerRewardPda: PublicKey;
  providerWallet: Keypair;
  providerUsdcAta: PublicKey;
  providerStakeUsdcVault: PublicKey;
  testUsdcMint: PublicKey;
}): Promise<Record<string, unknown>> {
  const {
    connection,
    masterpool,
    masterpoolConfigPda,
    providerAccountPda,
    providerRewardPda,
    providerWallet,
    providerUsdcAta,
    providerStakeUsdcVault,
    testUsdcMint,
  } = args;

  const existing = await connection.getAccountInfo(providerAccountPda, "confirmed");
  if (existing) {
    const providerState = await (masterpool.account as any).providerAccount.fetch(
      providerAccountPda
    );
    if (providerState.status !== 0) {
      throw new Error("provider account exists but is not Active");
    }

    return {
      action: "skipped",
      reason: "provider already active",
      providerAccountPda: providerAccountPda.toBase58(),
      providerRewardPda: providerRewardPda.toBase58(),
      providerUsdcToken: providerUsdcAta.toBase58(),
      stakedUsdcAmount: providerState.stakedUsdcAmount.toString(),
    };
  }

  const signature = await masterpool.methods
    .registerProvider()
    .accounts({
      config: masterpoolConfigPda,
      providerAccount: providerAccountPda,
      providerRewardAccount: providerRewardPda,
      providerWallet: providerWallet.publicKey,
      providerStakeUsdcVault,
      providerUsdcToken: providerUsdcAta,
      usdcMint: testUsdcMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([providerWallet])
    .rpc();

  const providerState = await (masterpool.account as any).providerAccount.fetch(
    providerAccountPda
  );
  return {
    action: "registered",
    signature,
    providerAccountPda: providerAccountPda.toBase58(),
    providerRewardPda: providerRewardPda.toBase58(),
    providerUsdcToken: providerUsdcAta.toBase58(),
    stakedUsdcAmount: providerState.stakedUsdcAmount.toString(),
    status: providerState.status,
  };
}

function makeSubmitArgs(args: {
  smokeConfig: SmokeTestConfig;
  requestNonce: string;
  providerWallet: PublicKey;
  payerUser: PublicKey;
  usdcMint: PublicKey;
}) {
  const { smokeConfig, requestNonce, providerWallet, payerUser, usdcMint } = args;
  const BN = ((anchor as any).BN ?? (anchor as any).default.BN) as typeof anchor.BN;
  const metadata = buildCompactReceiptMetadata({
    proofId: smokeConfig.proofId,
    providerCode: smokeConfig.providerCode,
    model: smokeConfig.model,
  });
  const compactArgs = buildCompactSubmitArgs(
    {
      requestNonce,
      metadata,
      providerWallet,
      payerUser,
      usdcMint,
      promptTokens: smokeConfig.promptTokens,
      completionTokens: smokeConfig.completionTokens,
      chargeAtomic: smokeConfig.receiptChargeBaseUnits,
    },
    (value) => new BN(value.toString())
  );

  return {
    requestNonce,
    compactArgs,
    receiptHashHex: receiptHashHex(Uint8Array.from(compactArgs.receiptHash)),
  };
}

async function submitReceipt(args: {
  provider: anchor.AnchorProvider;
  attestation: anchor.Program<any>;
  providerSignerSecretKey: Uint8Array;
  submitArgs: ReturnType<typeof makeSubmitArgs>;
  attestationConfigPda: PublicKey;
  providerSignerPda: PublicKey;
  payer: Keypair;
  payerUsdcAta: PublicKey;
  masterpoolConfigPda: PublicKey;
  masterpoolProgramId: PublicKey;
  providerAccountPda: PublicKey;
  providerRewardPda: PublicKey;
  userRewardPda: PublicKey;
  treasuryUsdcVaultPda: PublicKey;
  providerPendingUsdcVaultPda: PublicKey;
  usdcMint: PublicKey;
}): Promise<{ signature: string; receiptPda: PublicKey; settlementPda: PublicKey }> {
  const {
    provider,
    attestation,
    providerSignerSecretKey,
    submitArgs,
    attestationConfigPda,
    providerSignerPda,
    payer,
    payerUsdcAta,
    masterpoolConfigPda,
    masterpoolProgramId,
    providerAccountPda,
    providerRewardPda,
    userRewardPda,
    treasuryUsdcVaultPda,
    providerPendingUsdcVaultPda,
    usdcMint,
  } = args;

  const receiptPda = deriveReceiptPda(attestation.programId, submitArgs.requestNonce);
  const settlementPda = deriveReceiptSettlementPda(masterpoolProgramId, receiptPda);
  const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
    privateKey: providerSignerSecretKey,
    message: Uint8Array.from(submitArgs.compactArgs.receiptHash),
  });
  const submitIx = await attestation.methods
    .submitReceipt(submitArgs.compactArgs)
    .accounts({
      authority: provider.wallet.publicKey,
      config: attestationConfigPda,
      providerSigner: providerSignerPda,
      receipt: receiptPda,
      payerUser: payer.publicKey,
      payerUsdcToken: payerUsdcAta,
      masterpoolConfig: masterpoolConfigPda,
      masterpoolProgram: masterpoolProgramId,
      masterpoolProviderAccount: providerAccountPda,
      masterpoolProviderRewardAccount: providerRewardPda,
      masterpoolUserRewardAccount: userRewardPda,
      masterpoolReceiptSettlement: settlementPda,
      masterpoolTreasuryUsdcVault: treasuryUsdcVaultPda,
      masterpoolProviderPendingUsdcVault: providerPendingUsdcVaultPda,
      usdcMint,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .instruction();

  const tx = new Transaction().add(ed25519Ix, submitIx);
  const signature = await provider.sendAndConfirm(tx, [payer]);
  return { signature, receiptPda, settlementPda };
}

function deriveProviderSignerPda(
  attestationProgramId: PublicKey,
  providerWallet: PublicKey,
  signer: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("provider_signer"),
      providerWallet.toBuffer(),
      signer.toBuffer(),
    ],
    attestationProgramId
  )[0];
}

function deriveProviderAccountPda(
  masterpoolProgramId: PublicKey,
  providerWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("provider"), providerWallet.toBuffer()],
    masterpoolProgramId
  )[0];
}

function deriveProviderRewardAccountPda(
  masterpoolProgramId: PublicKey,
  providerWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("provider_reward"), providerWallet.toBuffer()],
    masterpoolProgramId
  )[0];
}

function deriveUserRewardAccountPda(
  masterpoolProgramId: PublicKey,
  userWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("user_reward"), userWallet.toBuffer()],
    masterpoolProgramId
  )[0];
}

function deriveReceiptPda(
  attestationProgramId: PublicKey,
  requestNonce: string
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("receipt"), hashRequestNonce(requestNonce)],
    attestationProgramId
  )[0];
}

function deriveReceiptSettlementPda(
  masterpoolProgramId: PublicKey,
  receiptPda: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("receipt_settlement"), receiptPda.toBuffer()],
    masterpoolProgramId
  )[0];
}

export function buildRequestNonce(
  prefix: string,
  suffix: string,
  now: number = Date.now()
): string {
  return `${prefix}-${suffix}-${now.toString(36)}`;
}

async function expectErrorContaining(
  action: () => Promise<unknown>,
  expected: string
): Promise<string> {
  try {
    await action();
  } catch (error) {
    const message = serializeError(error);
    if (message.includes(expected)) {
      return message;
    }
    throw new Error(`expected error containing ${expected}, got: ${message}`);
  }

  throw new Error(`expected error containing ${expected}`);
}

function calculateBpsAmount(amount: bigint, bps: bigint): bigint {
  return (amount * bps) / BPS_SCALE;
}

function assertPubkeyEquals(
  actual: PublicKey,
  expected: PublicKey | string,
  label: string
): void {
  const expectedKey =
    typeof expected === "string" ? new PublicKey(expected) : expected;
  if (!actual.equals(expectedKey)) {
    throw new Error(`${label} mismatch: ${actual.toBase58()} != ${expectedKey.toBase58()}`);
  }
}

function assertBigIntEquals(actual: bigint, expected: bigint, label: string): void {
  if (actual !== expected) {
    throw new Error(`${label} mismatch: ${actual.toString()} != ${expected.toString()}`);
  }
}

function requireNonEmptyString(value: string | undefined, label: string): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    throw new Error(`${label} is required`);
  }
  return trimmed;
}

function normalizeNonNegativeInteger(
  value: number | undefined,
  fallback: number,
  label: string
): number {
  const resolved = value ?? fallback;
  if (!Number.isSafeInteger(resolved) || resolved < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
  return resolved;
}

function normalizePositiveInteger(
  value: number | undefined,
  fallback: number,
  label: string
): number {
  const resolved = value ?? fallback;
  if (!Number.isSafeInteger(resolved) || resolved <= 0) {
    throw new Error(`${label} must be a positive safe integer`);
  }
  return resolved;
}

function normalizeCompactAlias(
  value: string | undefined,
  fallback: string,
  aliases: Array<[string, string]>
): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    return fallback;
  }
  for (const [input, normalized] of aliases) {
    if (trimmed === input) {
      return normalized;
    }
  }
  return trimmed;
}

async function writeJsonFile(outputPath: string, value: unknown): Promise<void> {
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function serializeError(error: unknown): string {
  if (error instanceof Error) {
    const maybeLogs = (error as Error & { logs?: string[] }).logs;
    if (Array.isArray(maybeLogs) && maybeLogs.length > 0) {
      return `${error.message}\n${maybeLogs.join("\n")}`;
    }
    return error.message;
  }
  return String(error);
}

function logStep(message: string): void {
  console.error(`[phase1-smoketest] ${message}`);
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
