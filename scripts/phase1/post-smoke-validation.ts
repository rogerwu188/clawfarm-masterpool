import * as anchor from "@coral-xyz/anchor";
import { promises as fs, readFileSync } from "fs";
import path from "path";
import {
  getAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
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
  buildCompactReceiptMetadata,
  buildCompactSubmitArgs,
  hashRequestNonce,
  receiptHashHex,
} from "./compact-receipt";
import {
  DeploymentRecord,
  deriveAttestationConfig,
  deriveMasterpoolPdas,
  loadKeypair,
  toBaseUnits,
} from "./common";

const DEFAULT_PROOF_ID = "r";
const DEFAULT_MODEL = "m";
const DEFAULT_PROMPT_TOKENS = 123;
const DEFAULT_COMPLETION_TOKENS = 456;
const CHALLENGE_TYPE_PAYLOAD_MISMATCH = 4;
const CHALLENGE_RESOLUTION_ACCEPTED = 1;
const CHALLENGE_RESOLUTION_REJECTED = 2;
const CHALLENGE_EVIDENCE_HASH = Array.from(new Uint8Array(32).fill(7));

type ChallengeCaseKind = "rejected" | "accepted" | "timeoutReject";

type ReportStatus = "ok" | "failed" | "waiting";

export interface PostSmokeValidationArgs {
  deployment: string;
  config: string;
  out?: string;
}

export interface RawPostSmokeChallengeCaseConfig {
  requestNonce?: string;
  providerCode?: string;
  proofId?: string;
  model?: string;
  receiptChargeUiAmount?: string;
  promptTokens?: number;
  completionTokens?: number;
}

export interface PostSmokeChallengeCaseConfig {
  requestNonce: string;
  providerCode: string;
  proofId: string;
  model: string;
  receiptChargeUiAmount: string;
  receiptChargeBaseUnits: bigint;
  promptTokens: number;
  completionTokens: number;
}

export interface RawPostSmokeChallengeCasesConfig {
  rejected?: RawPostSmokeChallengeCaseConfig;
  accepted?: RawPostSmokeChallengeCaseConfig;
  timeoutReject?: RawPostSmokeChallengeCaseConfig;
}

export interface PostSmokeChallengeCasesConfig {
  rejected?: PostSmokeChallengeCaseConfig;
  accepted?: PostSmokeChallengeCaseConfig;
  timeoutReject?: PostSmokeChallengeCaseConfig;
}

interface ChallengeReceiptDefaultsInput {
  providerCode?: string;
  proofId?: string;
  model?: string;
  receiptChargeUiAmount?: string;
  promptTokens?: number;
  completionTokens?: number;
}

export interface RawPostSmokeValidationConfig {
  adminKeypair?: string;
  payerKeypair?: string;
  providerWalletKeypair?: string;
  providerSignerKeypair?: string;
  receiptPda?: string;
  settlementPda?: string;
  providerCode?: string;
  proofId?: string;
  model?: string;
  receiptChargeUiAmount?: string;
  promptTokens?: number;
  completionTokens?: number;
  challengeCases?: RawPostSmokeChallengeCasesConfig;
}

export interface PostSmokeValidationConfig {
  adminKeypair: string;
  payerKeypair?: string;
  providerWalletKeypair?: string;
  providerSignerKeypair?: string;
  receiptPda: string;
  settlementPda: string;
  challengeCases?: PostSmokeChallengeCasesConfig;
}

export interface FinalizeWindowReport {
  canFinalizeNow: boolean;
  secondsUntilFinalizable: number;
  earliestFinalizeAtUnixSeconds: number;
  earliestFinalizeAtIso: string;
}

interface PostSmokeValidationReport {
  status: ReportStatus;
  startedAt: string;
  finishedAt?: string;
  deployment: string;
  config: string;
  steps: {
    chainConfig?: Record<string, unknown>;
    receiptBaseline?: Record<string, unknown>;
    finalize?: Record<string, unknown>;
    rewardRelease?: Record<string, unknown>;
    challenge?: Record<string, unknown>;
  };
  error?: string;
}

interface ChallengeContext {
  admin: Keypair;
  wallet: anchor.Wallet;
  provider: anchor.AnchorProvider;
  connection: anchor.web3.Connection;
  masterpool: anchor.Program<any>;
  attestation: anchor.Program<any>;
  masterpoolProgramId: PublicKey;
  attestationProgramId: PublicKey;
  clawMint: PublicKey;
  testUsdcMint: PublicKey;
  pdas: ReturnType<typeof deriveMasterpoolPdas>;
  attestationConfigPda: PublicKey;
  masterpoolConfig: any;
  attestationConfig: any;
  providerWallet: PublicKey;
  providerAccountPda: PublicKey;
  providerRewardPda: PublicKey;
  providerDestinationUsdc: PublicKey;
  providerSignerKeypair: Keypair;
  providerSignerPda: PublicKey;
  payerKeypair: Keypair;
  payerUsdcAta: PublicKey;
  payerClawAta: PublicKey;
  userRewardPda: PublicKey;
}

interface ChallengeCaseRunResult {
  status: ReportStatus;
  report: Record<string, unknown>;
}

export function parsePostSmokeValidationArgs(
  argv: string[]
): PostSmokeValidationArgs {
  const deployment = valueOf(argv, "--deployment");
  const config = valueOf(argv, "--config");
  const out = valueOf(argv, "--out");

  if (!deployment) throw new Error("deployment path is required");
  if (!config) throw new Error("config path is required");

  return { deployment, config, out };
}

export function normalizePostSmokeValidationConfig(
  raw: RawPostSmokeValidationConfig
): PostSmokeValidationConfig {
  const adminKeypair = requireNonEmptyString(raw.adminKeypair, "adminKeypair");
  const receiptPda = requireValidPublicKeyString(raw.receiptPda, "receiptPda");
  const settlementPda = requireValidPublicKeyString(
    raw.settlementPda,
    "settlementPda"
  );
  const challengeCases = normalizeChallengeCases(raw.challengeCases, {
    providerCode: raw.providerCode,
    proofId: raw.proofId,
    model: raw.model,
    receiptChargeUiAmount: raw.receiptChargeUiAmount,
    promptTokens: raw.promptTokens,
    completionTokens: raw.completionTokens,
  });
  const payerKeypair = normalizeOptionalPath(raw.payerKeypair);
  const providerSignerKeypair = normalizeOptionalPath(raw.providerSignerKeypair);

  if (challengeCases && !payerKeypair) {
    throw new Error("payerKeypair is required when challenge cases are enabled");
  }
  if (challengeCases && !providerSignerKeypair) {
    throw new Error(
      "providerSignerKeypair is required when challenge cases are enabled"
    );
  }

  return {
    adminKeypair,
    payerKeypair,
    providerWalletKeypair: normalizeOptionalPath(raw.providerWalletKeypair),
    providerSignerKeypair,
    receiptPda,
    settlementPda,
    challengeCases,
  };
}

export function normalizeChallengeCases(
  raw: RawPostSmokeChallengeCasesConfig | undefined,
  defaults: ChallengeReceiptDefaultsInput
): PostSmokeChallengeCasesConfig | undefined {
  if (!raw) {
    return undefined;
  }

  const normalized: PostSmokeChallengeCasesConfig = {};
  if (raw.rejected) {
    normalized.rejected = normalizeChallengeCase(
      raw.rejected,
      defaults,
      "rejected"
    );
  }
  if (raw.accepted) {
    normalized.accepted = normalizeChallengeCase(
      raw.accepted,
      defaults,
      "accepted"
    );
  }
  if (raw.timeoutReject) {
    normalized.timeoutReject = normalizeChallengeCase(
      raw.timeoutReject,
      defaults,
      "timeoutReject"
    );
  }

  if (
    !normalized.rejected &&
    !normalized.accepted &&
    !normalized.timeoutReject
  ) {
    return undefined;
  }

  return normalized;
}

export function advanceReportStatus(
  current: ReportStatus,
  next: ReportStatus
): ReportStatus {
  const priority: Record<ReportStatus, number> = {
    ok: 0,
    waiting: 1,
    failed: 2,
  };
  return priority[next] > priority[current] ? next : current;
}

export function computeFinalizeWindow(args: {
  challengeDeadlineUnixSeconds: number;
  nowUnixSeconds: number;
}): FinalizeWindowReport {
  const earliestFinalizeAtUnixSeconds = args.challengeDeadlineUnixSeconds + 1;
  const secondsUntilFinalizable = Math.max(
    earliestFinalizeAtUnixSeconds - args.nowUnixSeconds,
    0
  );

  return {
    canFinalizeNow: secondsUntilFinalizable === 0,
    secondsUntilFinalizable,
    earliestFinalizeAtUnixSeconds,
    earliestFinalizeAtIso: new Date(
      earliestFinalizeAtUnixSeconds * 1000
    ).toISOString(),
  };
}

export function validateMainnetFaucetDisabled(
  cluster: string,
  faucetConfig: { enabled: boolean } | null
): void {
  if (cluster !== "mainnet-beta") return;
  if (!faucetConfig) return;
  if (faucetConfig.enabled) {
    throw new Error("mainnet faucet must not be enabled");
  }
}

export function computeLinearReleasableAmount(args: {
  totalLocked: number;
  releasedSoFar: number;
  lockStartUnixSeconds: number;
  nowUnixSeconds: number;
  lockDays: number;
}): number {
  const lockSeconds = args.lockDays * 86_400;
  if (!Number.isSafeInteger(lockSeconds) || lockSeconds <= 0) {
    throw new Error("lockDays must produce a positive safe lock duration");
  }

  const elapsed = Math.max(
    0,
    Math.min(args.nowUnixSeconds - args.lockStartUnixSeconds, lockSeconds)
  );
  const vested = Math.floor((args.totalLocked * elapsed) / lockSeconds);
  return vested - args.releasedSoFar;
}

export function derivePostSmokeFinalizeTargets(args: {
  masterpoolProgramId: PublicKey;
  providerWallet: PublicKey;
  payerUser: PublicKey;
  usdcMint: PublicKey;
}) {
  const [providerAccountPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("provider"), args.providerWallet.toBuffer()],
    args.masterpoolProgramId
  );
  const [providerRewardPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("provider_reward"), args.providerWallet.toBuffer()],
    args.masterpoolProgramId
  );
  const [userRewardPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("user_reward"), args.payerUser.toBuffer()],
    args.masterpoolProgramId
  );

  return {
    providerAccountPda,
    providerRewardPda,
    userRewardPda,
    providerDestinationUsdc: getAssociatedTokenAddressSync(
      args.usdcMint,
      args.providerWallet
    ),
  };
}

export function deriveChallengeCaseTargets(args: {
  attestationProgramId: PublicKey;
  masterpoolProgramId: PublicKey;
  requestNonce: string;
}) {
  const [receiptPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("receipt"), hashRequestNonce(args.requestNonce)],
    args.attestationProgramId
  );
  const [settlementPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("receipt_settlement"), receiptPda.toBuffer()],
    args.masterpoolProgramId
  );
  const [challengePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("challenge"), receiptPda.toBuffer()],
    args.attestationProgramId
  );
  const [challengeBondRecordPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("challenge_bond_record"), challengePda.toBuffer()],
    args.masterpoolProgramId
  );

  return {
    receiptPda,
    settlementPda,
    challengePda,
    challengeBondRecordPda,
  };
}

async function loadPostSmokeValidationConfig(
  configPath: string
): Promise<PostSmokeValidationConfig> {
  const raw = JSON.parse(
    await fs.readFile(configPath, "utf8")
  ) as RawPostSmokeValidationConfig;
  return normalizePostSmokeValidationConfig(raw);
}

function usage(): string {
  return [
    "Usage: yarn phase1:post-smoke:devnet --deployment <path> --config <path> [--out <path>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --config",
    "",
    "Optional flags:",
    "  --out reports/phase1-post-smoke-validation.json",
  ].join("\n");
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parsePostSmokeValidationArgs(argv);
  const report: PostSmokeValidationReport = {
    status: "failed",
    startedAt: new Date().toISOString(),
    deployment: args.deployment,
    config: args.config,
    steps: {},
  };

  try {
    report.status = "ok";

    const deployment = JSON.parse(
      await fs.readFile(args.deployment, "utf8")
    ) as DeploymentRecord;
    const validationConfig = await loadPostSmokeValidationConfig(args.config);

    logStep("loading admin and programs");
    const admin = await loadKeypair(validationConfig.adminKeypair);
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

    const receiptPda = new PublicKey(validationConfig.receiptPda);
    const settlementPda = new PublicKey(validationConfig.settlementPda);
    const attestationConfigPda = deriveAttestationConfig(attestationProgramId);
    const pdas = deriveMasterpoolPdas(masterpoolProgramId);

    const [masterpoolConfig, attestationConfig] = await Promise.all([
      (masterpool.account as any).globalConfig.fetch(pdas.masterpoolConfig),
      (attestation.account as any).config.fetch(attestationConfigPda),
    ]);
    const faucetConfig = await fetchNullableProgramAccount(
      connection,
      (masterpool.account as any).faucetConfig,
      pdas.faucetConfig
    );
    validateMainnetFaucetDisabled(deployment.cluster, faucetConfig);

    logStep("loading receipt and settlement baseline");
    const receipt = await (attestation.account as any).receipt.fetch(receiptPda);
    const settlement = await (masterpool.account as any).receiptSettlement.fetch(
      settlementPda
    );
    const attestationReceiptFromSettlement = new PublicKey(
      settlement.attestationReceipt
    );
    if (!attestationReceiptFromSettlement.equals(receiptPda)) {
      throw new Error(
        `settlement receipt mismatch: ${attestationReceiptFromSettlement.toBase58()} != ${receiptPda.toBase58()}`
      );
    }

    const providerWallet = new PublicKey(receipt.providerWallet);
    const payerUser = new PublicKey(receipt.payerUser);
    const finalizeTargets = derivePostSmokeFinalizeTargets({
      masterpoolProgramId,
      providerWallet,
      payerUser,
      usdcMint: testUsdcMint,
    });

    const challengeDeadlineUnixSeconds = toSafeNumber(
      receipt.challengeDeadline,
      "receipt.challengeDeadline"
    );
    const nowUnixSeconds = Math.floor(Date.now() / 1000);
    const finalizeWindow = computeFinalizeWindow({
      challengeDeadlineUnixSeconds,
      nowUnixSeconds,
    });

    const providerUsdcBefore = (await getAccount(
      connection,
      finalizeTargets.providerDestinationUsdc
    )).amount;
    const providerPendingBefore = (
      await getAccount(connection, pdas.providerPendingUsdcVault)
    ).amount;

    const receiptStatusBefore = Number(receipt.status);
    const settlementStatusBefore = Number(settlement.status);
    const providerShareAtomic = toBigIntString(
      settlement.usdcToProvider,
      "settlement.usdcToProvider"
    );

    report.steps.chainConfig = {
      masterpoolProgramId: masterpoolProgramId.toBase58(),
      attestationProgramId: attestationProgramId.toBase58(),
      masterpoolConfig: pdas.masterpoolConfig.toBase58(),
      attestationConfig: attestationConfigPda.toBase58(),
      testUsdcMint: testUsdcMint.toBase58(),
      providerPendingUsdcVault: pdas.providerPendingUsdcVault.toBase58(),
      challengeWindowSeconds: toSafeNumber(
        attestationConfig.challengeWindowSeconds,
        "attestationConfig.challengeWindowSeconds"
      ),
      challengeResolutionTimeoutSeconds: toSafeNumber(
        attestationConfig.challengeResolutionTimeoutSeconds,
        "attestationConfig.challengeResolutionTimeoutSeconds"
      ),
      challengeBondClawAmount: toBigIntString(
        masterpoolConfig.challengeBondClawAmount,
        "masterpoolConfig.challengeBondClawAmount"
      ),
    };
    report.steps.receiptBaseline = {
      receiptPda: receiptPda.toBase58(),
      settlementPda: settlementPda.toBase58(),
      providerWallet: providerWallet.toBase58(),
      payerUser: payerUser.toBase58(),
      providerDestinationUsdc: finalizeTargets.providerDestinationUsdc.toBase58(),
      challengeDeadlineUnixSeconds,
      challengeDeadlineIso: new Date(
        challengeDeadlineUnixSeconds * 1000
      ).toISOString(),
      canFinalizeNow: finalizeWindow.canFinalizeNow,
      secondsUntilFinalizable: finalizeWindow.secondsUntilFinalizable,
      earliestFinalizeAtUnixSeconds: finalizeWindow.earliestFinalizeAtUnixSeconds,
      earliestFinalizeAtIso: finalizeWindow.earliestFinalizeAtIso,
      receiptStatusBefore,
      settlementStatusBefore,
      providerUsdcBefore: providerUsdcBefore.toString(),
      providerPendingVaultBefore: providerPendingBefore.toString(),
      expectedProviderShareAtomic: providerShareAtomic,
    };

    if (receiptStatusBefore === 2 && settlementStatusBefore === 1) {
      report.steps.finalize = {
        action: "skipped",
        reason: "receipt already finalized and settled",
      };
    } else if (!finalizeWindow.canFinalizeNow) {
      report.steps.finalize = {
        action: "waiting",
        reason: "receipt challenge window still open",
        secondsUntilFinalizable: finalizeWindow.secondsUntilFinalizable,
        earliestFinalizeAtUnixSeconds:
          finalizeWindow.earliestFinalizeAtUnixSeconds,
        earliestFinalizeAtIso: finalizeWindow.earliestFinalizeAtIso,
      };
      report.status = advanceReportStatus(report.status, "waiting");
    } else {
      logStep("finalizing smoke receipt");
      const signature = await finalizeReceiptWithTargets({
        attestation,
        authority: wallet.publicKey,
        attestationConfigPda,
        receiptPda,
        masterpool,
        pdas,
        settlementPda,
        providerAccountPda: finalizeTargets.providerAccountPda,
        providerRewardPda: finalizeTargets.providerRewardPda,
        userRewardPda: finalizeTargets.userRewardPda,
        providerDestinationUsdc: finalizeTargets.providerDestinationUsdc,
        usdcMint: testUsdcMint,
      });

      const receiptAfter = await (attestation.account as any).receipt.fetch(receiptPda);
      const settlementAfter = await (masterpool.account as any).receiptSettlement.fetch(
        settlementPda
      );
      const providerUsdcAfter = (await getAccount(
        connection,
        finalizeTargets.providerDestinationUsdc
      )).amount;
      const providerPendingAfter = (
        await getAccount(connection, pdas.providerPendingUsdcVault)
      ).amount;

      const expectedProviderDelta = BigInt(providerShareAtomic);
      const actualProviderDelta = providerUsdcAfter - providerUsdcBefore;
      const actualPendingDelta = providerPendingBefore - providerPendingAfter;

      assertBigIntEquals(
        actualProviderDelta,
        expectedProviderDelta,
        "provider usdc delta"
      );
      assertBigIntEquals(
        actualPendingDelta,
        expectedProviderDelta,
        "provider pending vault delta"
      );

      report.steps.finalize = {
        action: "finalized",
        signature,
        providerUsdcBefore: providerUsdcBefore.toString(),
        providerUsdcAfter: providerUsdcAfter.toString(),
        providerPendingVaultBefore: providerPendingBefore.toString(),
        providerPendingVaultAfter: providerPendingAfter.toString(),
        expectedProviderDelta: expectedProviderDelta.toString(),
        receiptStatusAfter: Number(receiptAfter.status),
        settlementStatusAfter: Number(settlementAfter.status),
        receiptFinalizedAt: toSafeNumber(
          receiptAfter.finalizedAt,
          "receiptAfter.finalizedAt"
        ),
        rewardLockStartedAt: toSafeNumber(
          settlementAfter.rewardLockStartedAt,
          "settlementAfter.rewardLockStartedAt"
        ),
      };
    }

    const settlementForRewards = await (masterpool.account as any).receiptSettlement.fetch(
      settlementPda
    );
    if (Number(settlementForRewards.status) === 1) {
      logStep("evaluating reward release and claim");
      const chainNowUnixSeconds = await currentUnixTime(connection);
      const userRewardAccount = await (masterpool.account as any).rewardAccount.fetch(
        finalizeTargets.userRewardPda
      );
      const providerRewardAccount = await (masterpool.account as any).rewardAccount.fetch(
        finalizeTargets.providerRewardPda
      );

      const userReleasable = computeLinearReleasableAmount({
        totalLocked: decodeU64(settlementForRewards.clawToUser),
        releasedSoFar: decodeU64(settlementForRewards.userClawReleased),
        lockStartUnixSeconds: toSafeNumber(
          settlementForRewards.rewardLockStartedAt,
          "settlement.rewardLockStartedAt"
        ),
        nowUnixSeconds: chainNowUnixSeconds,
        lockDays: Number(settlementForRewards.lockDaysSnapshot),
      });
      const providerReleasable = computeLinearReleasableAmount({
        totalLocked: decodeU64(settlementForRewards.clawToProviderLocked),
        releasedSoFar: decodeU64(settlementForRewards.providerClawReleased),
        lockStartUnixSeconds: toSafeNumber(
          settlementForRewards.rewardLockStartedAt,
          "settlement.rewardLockStartedAt"
        ),
        nowUnixSeconds: chainNowUnixSeconds,
        lockDays: Number(settlementForRewards.lockDaysSnapshot),
      });

      report.steps.rewardRelease = {
        user: {
          rewardAccount: finalizeTargets.userRewardPda.toBase58(),
          claimant: payerUser.toBase58(),
          releasableNow: String(userReleasable),
          lockedBefore: toBigIntString(
            userRewardAccount.lockedClawTotal,
            "userRewardAccount.lockedClawTotal"
          ),
          releasedBefore: toBigIntString(
            userRewardAccount.releasedClawTotal,
            "userRewardAccount.releasedClawTotal"
          ),
          claimedBefore: toBigIntString(
            userRewardAccount.claimedClawTotal,
            "userRewardAccount.claimedClawTotal"
          ),
        },
        provider: {
          rewardAccount: finalizeTargets.providerRewardPda.toBase58(),
          claimant: providerWallet.toBase58(),
          releasableNow: String(providerReleasable),
          lockedBefore: toBigIntString(
            providerRewardAccount.lockedClawTotal,
            "providerRewardAccount.lockedClawTotal"
          ),
          releasedBefore: toBigIntString(
            providerRewardAccount.releasedClawTotal,
            "providerRewardAccount.releasedClawTotal"
          ),
          claimedBefore: toBigIntString(
            providerRewardAccount.claimedClawTotal,
            "providerRewardAccount.claimedClawTotal"
          ),
        },
      };

      if (userReleasable > 0) {
        if (!validationConfig.payerKeypair) {
          (report.steps.rewardRelease.user as Record<string, unknown>).action = "skipped";
          (report.steps.rewardRelease.user as Record<string, unknown>).reason =
            "payerKeypair is required to claim user rewards";
        } else {
          const payer = await loadKeypair(validationConfig.payerKeypair);
          if (!payer.publicKey.equals(payerUser)) {
            throw new Error(
              `payerKeypair does not match receipt payerUser: ${payer.publicKey.toBase58()} != ${payerUser.toBase58()}`
            );
          }
          const payerClawToken = (
            await getOrCreateAssociatedTokenAccount(
              connection,
              admin,
              clawMint,
              payerUser
            )
          ).address;

          const releaseSignature = await masterpool.methods
            .materializeRewardRelease(0, new anchor.BN(userReleasable))
            .accounts({
              config: pdas.masterpoolConfig,
              adminAuthority: wallet.publicKey,
              rewardAccount: finalizeTargets.userRewardPda,
              receiptSettlement: settlementPda,
            } as any)
            .rpc();
          const claimSignature = await masterpool.methods
            .claimReleasedClaw()
            .accounts({
              config: pdas.masterpoolConfig,
              rewardAccount: finalizeTargets.userRewardPda,
              claimant: payerUser,
              rewardVault: pdas.rewardVault,
              claimantClawToken: payerClawToken,
              clawMint,
              poolAuthority: pdas.poolAuthority,
              tokenProgram: TOKEN_PROGRAM_ID,
            } as any)
            .signers([payer])
            .rpc();
          const payerClawAfter = await getAccount(connection, payerClawToken);
          Object.assign(report.steps.rewardRelease.user as Record<string, unknown>, {
            action: "released_and_claimed",
            releaseSignature,
            claimSignature,
            claimantClawToken: payerClawToken.toBase58(),
            claimantClawAfter: payerClawAfter.amount.toString(),
          });
        }
      } else {
        (report.steps.rewardRelease.user as Record<string, unknown>).action = "skipped";
        (report.steps.rewardRelease.user as Record<string, unknown>).reason =
          "no vested user rewards yet";
      }

      if (providerReleasable > 0) {
        if (!validationConfig.providerWalletKeypair) {
          (report.steps.rewardRelease.provider as Record<string, unknown>).action =
            "skipped";
          (report.steps.rewardRelease.provider as Record<string, unknown>).reason =
            "providerWalletKeypair is required to claim provider rewards";
        } else {
          const providerWalletKeypair = await loadKeypair(
            validationConfig.providerWalletKeypair
          );
          if (!providerWalletKeypair.publicKey.equals(providerWallet)) {
            throw new Error(
              `providerWalletKeypair does not match receipt providerWallet: ${providerWalletKeypair.publicKey.toBase58()} != ${providerWallet.toBase58()}`
            );
          }
          const providerClawToken = (
            await getOrCreateAssociatedTokenAccount(
              connection,
              admin,
              clawMint,
              providerWallet
            )
          ).address;

          const releaseSignature = await masterpool.methods
            .materializeRewardRelease(1, new anchor.BN(providerReleasable))
            .accounts({
              config: pdas.masterpoolConfig,
              adminAuthority: wallet.publicKey,
              rewardAccount: finalizeTargets.providerRewardPda,
              receiptSettlement: settlementPda,
            } as any)
            .rpc();
          const claimSignature = await masterpool.methods
            .claimReleasedClaw()
            .accounts({
              config: pdas.masterpoolConfig,
              rewardAccount: finalizeTargets.providerRewardPda,
              claimant: providerWallet,
              rewardVault: pdas.rewardVault,
              claimantClawToken: providerClawToken,
              clawMint,
              poolAuthority: pdas.poolAuthority,
              tokenProgram: TOKEN_PROGRAM_ID,
            } as any)
            .signers([providerWalletKeypair])
            .rpc();
          const providerClawAfter = await getAccount(connection, providerClawToken);
          Object.assign(report.steps.rewardRelease.provider as Record<string, unknown>, {
            action: "released_and_claimed",
            releaseSignature,
            claimSignature,
            claimantClawToken: providerClawToken.toBase58(),
            claimantClawAfter: providerClawAfter.amount.toString(),
          });
        }
      } else {
        (report.steps.rewardRelease.provider as Record<string, unknown>).action =
          "skipped";
        (report.steps.rewardRelease.provider as Record<string, unknown>).reason =
          "no vested provider rewards yet";
      }
    } else {
      report.steps.rewardRelease = {
        action: "skipped",
        reason: "receipt settlement not finalized",
        settlementStatus: Number(settlementForRewards.status),
      };
    }

    if (validationConfig.challengeCases) {
      logStep("running challenge validation paths");
      const challengeContext = await buildChallengeContext({
        admin,
        wallet,
        provider,
        connection,
        masterpool,
        attestation,
        masterpoolProgramId,
        attestationProgramId,
        clawMint,
        testUsdcMint,
        pdas,
        attestationConfigPda,
        masterpoolConfig,
        attestationConfig,
        providerWallet,
        providerDestinationUsdc: finalizeTargets.providerDestinationUsdc,
        providerAccountPda: finalizeTargets.providerAccountPda,
        providerRewardPda: finalizeTargets.providerRewardPda,
        providerSignerKeypairPath: validationConfig.providerSignerKeypair!,
        payerKeypairPath: validationConfig.payerKeypair!,
      });

      const challengeReport: Record<string, unknown> = {};
      for (const kind of [
        "rejected",
        "accepted",
        "timeoutReject",
      ] as const) {
        const caseConfig = validationConfig.challengeCases[kind];
        if (!caseConfig) {
          continue;
        }
        const result = await runChallengeCase(kind, caseConfig, challengeContext);
        challengeReport[kind] = result.report;
        report.status = advanceReportStatus(report.status, result.status);
      }
      report.steps.challenge = challengeReport;
    }
  } catch (error) {
    report.status = "failed";
    report.error = serializeError(error);
  } finally {
    report.finishedAt = new Date().toISOString();

    if (args.out) {
      await writeJsonFile(args.out, report);
      console.error(`wrote post-smoke report to ${args.out}`);
    }

    console.log(JSON.stringify(report, null, 2));
  }

  if (report.status === "failed") {
    throw new Error(report.error ?? "post-smoke validation failed");
  }
}

async function buildChallengeContext(args: {
  admin: Keypair;
  wallet: anchor.Wallet;
  provider: anchor.AnchorProvider;
  connection: anchor.web3.Connection;
  masterpool: anchor.Program<any>;
  attestation: anchor.Program<any>;
  masterpoolProgramId: PublicKey;
  attestationProgramId: PublicKey;
  clawMint: PublicKey;
  testUsdcMint: PublicKey;
  pdas: ReturnType<typeof deriveMasterpoolPdas>;
  attestationConfigPda: PublicKey;
  masterpoolConfig: any;
  attestationConfig: any;
  providerWallet: PublicKey;
  providerDestinationUsdc: PublicKey;
  providerAccountPda: PublicKey;
  providerRewardPda: PublicKey;
  providerSignerKeypairPath: string;
  payerKeypairPath: string;
}): Promise<ChallengeContext> {
  const providerSignerKeypair = await loadKeypair(args.providerSignerKeypairPath);
  const payerKeypair = await loadKeypair(args.payerKeypairPath);
  const providerSignerPda = deriveProviderSignerPda(
    args.attestationProgramId,
    args.providerWallet,
    providerSignerKeypair.publicKey
  );

  if (!(await args.connection.getAccountInfo(providerSignerPda))) {
    throw new Error(
      `provider signer PDA does not exist on chain: ${providerSignerPda.toBase58()}`
    );
  }

  const payerUsdcAta = (
    await getOrCreateAssociatedTokenAccount(
      args.connection,
      args.admin,
      args.testUsdcMint,
      payerKeypair.publicKey
    )
  ).address;
  const payerClawAta = (
    await getOrCreateAssociatedTokenAccount(
      args.connection,
      args.admin,
      args.clawMint,
      payerKeypair.publicKey
    )
  ).address;
  const [userRewardPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("user_reward"), payerKeypair.publicKey.toBuffer()],
    args.masterpoolProgramId
  );

  return {
    admin: args.admin,
    wallet: args.wallet,
    provider: args.provider,
    connection: args.connection,
    masterpool: args.masterpool,
    attestation: args.attestation,
    masterpoolProgramId: args.masterpoolProgramId,
    attestationProgramId: args.attestationProgramId,
    clawMint: args.clawMint,
    testUsdcMint: args.testUsdcMint,
    pdas: args.pdas,
    attestationConfigPda: args.attestationConfigPda,
    masterpoolConfig: args.masterpoolConfig,
    attestationConfig: args.attestationConfig,
    providerWallet: args.providerWallet,
    providerAccountPda: args.providerAccountPda,
    providerRewardPda: args.providerRewardPda,
    providerDestinationUsdc: args.providerDestinationUsdc,
    providerSignerKeypair,
    providerSignerPda,
    payerKeypair,
    payerUsdcAta,
    payerClawAta,
    userRewardPda,
  };
}

async function runChallengeCase(
  kind: ChallengeCaseKind,
  config: PostSmokeChallengeCaseConfig,
  context: ChallengeContext
): Promise<ChallengeCaseRunResult> {
  const targets = deriveChallengeCaseTargets({
    attestationProgramId: context.attestationProgramId,
    masterpoolProgramId: context.masterpoolProgramId,
    requestNonce: config.requestNonce,
  });
  const report: Record<string, unknown> = {
    requestNonce: config.requestNonce,
    receiptPda: targets.receiptPda.toBase58(),
    settlementPda: targets.settlementPda.toBase58(),
    challengePda: targets.challengePda.toBase58(),
    challengeBondRecordPda: targets.challengeBondRecordPda.toBase58(),
  };

  await assertTokenBalanceAtLeast(
    context.connection,
    context.payerUsdcAta,
    config.receiptChargeBaseUnits,
    "payer USDC balance"
  );
  await assertTokenBalanceAtLeast(
    context.connection,
    context.payerClawAta,
    BigInt(
      toBigIntString(
        context.masterpoolConfig.challengeBondClawAmount,
        "masterpoolConfig.challengeBondClawAmount"
      )
    ),
    "challenger CLAW balance"
  );

  const existingReceipt = await fetchMaybeProgramAccount(
    context.connection,
    (context.attestation.account as any).receipt,
    targets.receiptPda
  );
  if (!existingReceipt) {
    logStep(`submitting ${kind} challenge receipt`);
    const submitted = await submitChallengeReceipt(context, config, targets);
    report.submission = {
      action: "submitted",
      signature: submitted.signature,
      receiptHashHex: submitted.receiptHashHex,
    };
  } else {
    report.submission = {
      action: "skipped",
      reason: "receipt already exists",
    };
  }

  const settlementAfterSubmit = await (context.masterpool.account as any).receiptSettlement.fetch(
    targets.settlementPda
  );
  const receiptAfterSubmit = await (context.attestation.account as any).receipt.fetch(
    targets.receiptPda
  );
  Object.assign(report, {
    receiptStatusBeforeChallenge: Number(receiptAfterSubmit.status),
    settlementStatusBeforeChallenge: Number(settlementAfterSubmit.status),
  });

  const existingChallenge = await fetchMaybeProgramAccount(
    context.connection,
    (context.attestation.account as any).challenge,
    targets.challengePda
  );
  if (!existingChallenge) {
    logStep(`opening ${kind} challenge`);
    const signature = await openChallengeCase(context, targets);
    report.challengeOpen = {
      action: "opened",
      signature,
    };
  } else {
    report.challengeOpen = {
      action: "skipped",
      reason: "challenge already exists",
      status: Number(existingChallenge.status),
    };
  }

  const challenge = await (context.attestation.account as any).challenge.fetch(
    targets.challengePda
  );
  const challengeOpenedAt = toSafeNumber(challenge.openedAt, "challenge.openedAt");
  const challengeStatus = Number(challenge.status);

  if (kind === "rejected") {
    if (challengeStatus === 0) {
      logStep("resolving rejected challenge");
      const signature = await resolveChallengeCase(
        context,
        targets,
        CHALLENGE_RESOLUTION_REJECTED
      );
      report.challengeResolution = {
        action: "resolved",
        resolutionCode: CHALLENGE_RESOLUTION_REJECTED,
        signature,
      };
    } else {
      report.challengeResolution = {
        action: "skipped",
        reason: "challenge already resolved",
        status: challengeStatus,
      };
    }

    const settlement = await (context.masterpool.account as any).receiptSettlement.fetch(
      targets.settlementPda
    );
    if (Number(settlement.status) !== 1) {
      logStep("finalizing rejected challenge receipt");
      const signature = await finalizeReceiptWithTargets({
        attestation: context.attestation,
        authority: context.wallet.publicKey,
        attestationConfigPda: context.attestationConfigPda,
        receiptPda: targets.receiptPda,
        masterpool: context.masterpool,
        pdas: context.pdas,
        settlementPda: targets.settlementPda,
        providerAccountPda: context.providerAccountPda,
        providerRewardPda: context.providerRewardPda,
        userRewardPda: context.userRewardPda,
        providerDestinationUsdc: context.providerDestinationUsdc,
        usdcMint: context.testUsdcMint,
      });
      report.finalize = {
        action: "finalized",
        signature,
      };
    } else {
      report.finalize = {
        action: "skipped",
        reason: "settlement already finalized",
      };
    }

    const [receiptFinal, settlementFinal, challengeFinal, bondRecord] = await Promise.all([
      (context.attestation.account as any).receipt.fetch(targets.receiptPda),
      (context.masterpool.account as any).receiptSettlement.fetch(
        targets.settlementPda
      ),
      (context.attestation.account as any).challenge.fetch(targets.challengePda),
      (context.masterpool.account as any).challengeBondRecord.fetch(
        targets.challengeBondRecordPda
      ),
    ]);
    Object.assign(report, {
      receiptStatusAfter: Number(receiptFinal.status),
      settlementStatusAfter: Number(settlementFinal.status),
      challengeStatusAfter: Number(challengeFinal.status),
      challengeBondRecordStatusAfter: Number(bondRecord.status),
    });

    return { status: "ok", report };
  }

  if (kind === "accepted") {
    if (challengeStatus === 0) {
      logStep("resolving accepted challenge");
      const signature = await resolveChallengeCase(
        context,
        targets,
        CHALLENGE_RESOLUTION_ACCEPTED
      );
      report.challengeResolution = {
        action: "resolved",
        resolutionCode: CHALLENGE_RESOLUTION_ACCEPTED,
        signature,
      };
    } else {
      report.challengeResolution = {
        action: "skipped",
        reason: "challenge already resolved",
        status: challengeStatus,
      };
    }

    const [receiptFinal, settlementFinal, challengeFinal, bondRecord] = await Promise.all([
      (context.attestation.account as any).receipt.fetch(targets.receiptPda),
      (context.masterpool.account as any).receiptSettlement.fetch(
        targets.settlementPda
      ),
      (context.attestation.account as any).challenge.fetch(targets.challengePda),
      (context.masterpool.account as any).challengeBondRecord.fetch(
        targets.challengeBondRecordPda
      ),
    ]);
    const finalizeError = await expectErrorContaining(
      () =>
        finalizeReceiptWithTargets({
          attestation: context.attestation,
          authority: context.wallet.publicKey,
          attestationConfigPda: context.attestationConfigPda,
          receiptPda: targets.receiptPda,
          masterpool: context.masterpool,
          pdas: context.pdas,
          settlementPda: targets.settlementPda,
          providerAccountPda: context.providerAccountPda,
          providerRewardPda: context.providerRewardPda,
          userRewardPda: context.userRewardPda,
          providerDestinationUsdc: context.providerDestinationUsdc,
          usdcMint: context.testUsdcMint,
        }),
      "ReceiptNotFinalizable"
    );

    Object.assign(report, {
      receiptStatusAfter: Number(receiptFinal.status),
      settlementStatusAfter: Number(settlementFinal.status),
      challengeStatusAfter: Number(challengeFinal.status),
      challengeBondRecordStatusAfter: Number(bondRecord.status),
      finalizeCheck: {
        action: "rejected_as_expected",
        matchedError: "ReceiptNotFinalizable",
        error: finalizeError,
      },
    });

    return { status: "ok", report };
  }

  const timeoutWindow = computeFinalizeWindow({
    challengeDeadlineUnixSeconds:
      challengeOpenedAt +
      toSafeNumber(
        context.attestationConfig.challengeResolutionTimeoutSeconds,
        "attestationConfig.challengeResolutionTimeoutSeconds"
      ),
    nowUnixSeconds: await currentUnixTime(context.connection),
  });

  if (challengeStatus === 0 && !timeoutWindow.canFinalizeNow) {
    report.challengeTimeout = {
      action: "waiting",
      reason: "challenge resolution timeout still open",
      secondsUntilTimeoutReject: timeoutWindow.secondsUntilFinalizable,
      earliestTimeoutRejectAtUnixSeconds:
        timeoutWindow.earliestFinalizeAtUnixSeconds,
      earliestTimeoutRejectAtIso: timeoutWindow.earliestFinalizeAtIso,
    };
    return { status: "waiting", report };
  }

  if (challengeStatus === 0) {
    logStep("timing out open challenge");
    const signature = await timeoutRejectChallengeCase(context, targets);
    report.challengeTimeout = {
      action: "timed_out_and_rejected",
      signature,
    };
  } else {
    report.challengeTimeout = {
      action: "skipped",
      reason: "challenge already resolved",
      status: challengeStatus,
    };
  }

  const settlement = await (context.masterpool.account as any).receiptSettlement.fetch(
    targets.settlementPda
  );
  if (Number(settlement.status) !== 1) {
    logStep("finalizing timeout-rejected receipt");
    const signature = await finalizeReceiptWithTargets({
      attestation: context.attestation,
      authority: context.wallet.publicKey,
      attestationConfigPda: context.attestationConfigPda,
      receiptPda: targets.receiptPda,
      masterpool: context.masterpool,
      pdas: context.pdas,
      settlementPda: targets.settlementPda,
      providerAccountPda: context.providerAccountPda,
      providerRewardPda: context.providerRewardPda,
      userRewardPda: context.userRewardPda,
      providerDestinationUsdc: context.providerDestinationUsdc,
      usdcMint: context.testUsdcMint,
    });
    report.finalize = {
      action: "finalized",
      signature,
    };
  } else {
    report.finalize = {
      action: "skipped",
      reason: "settlement already finalized",
    };
  }

  const [receiptFinal, settlementFinal, challengeFinal, bondRecord] = await Promise.all([
    (context.attestation.account as any).receipt.fetch(targets.receiptPda),
    (context.masterpool.account as any).receiptSettlement.fetch(
      targets.settlementPda
    ),
    (context.attestation.account as any).challenge.fetch(targets.challengePda),
    (context.masterpool.account as any).challengeBondRecord.fetch(
      targets.challengeBondRecordPda
    ),
  ]);
  Object.assign(report, {
    receiptStatusAfter: Number(receiptFinal.status),
    settlementStatusAfter: Number(settlementFinal.status),
    challengeStatusAfter: Number(challengeFinal.status),
    challengeBondRecordStatusAfter: Number(bondRecord.status),
  });

  return { status: "ok", report };
}

async function submitChallengeReceipt(
  context: ChallengeContext,
  config: PostSmokeChallengeCaseConfig,
  targets: ReturnType<typeof deriveChallengeCaseTargets>
): Promise<{ signature: string; receiptHashHex: string }> {
  const BN = ((anchor as any).BN ?? (anchor as any).default.BN) as typeof anchor.BN;
  const metadata = buildCompactReceiptMetadata({
    proofId: config.proofId,
    providerCode: config.providerCode,
    model: config.model,
  });
  const compactArgs = buildCompactSubmitArgs(
    {
      requestNonce: config.requestNonce,
      metadata,
      providerWallet: context.providerWallet,
      payerUser: context.payerKeypair.publicKey,
      usdcMint: context.testUsdcMint,
      promptTokens: config.promptTokens,
      completionTokens: config.completionTokens,
      chargeAtomic: config.receiptChargeBaseUnits,
    },
    (value) => new BN(value.toString())
  );
  const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
    privateKey: context.providerSignerKeypair.secretKey,
    message: Uint8Array.from(compactArgs.receiptHash),
  });
  const submitIx = await context.attestation.methods
    .submitReceipt(compactArgs)
    .accounts({
      authority: context.wallet.publicKey,
      config: context.attestationConfigPda,
      providerSigner: context.providerSignerPda,
      receipt: targets.receiptPda,
      payerUser: context.payerKeypair.publicKey,
      payerUsdcToken: context.payerUsdcAta,
      masterpoolConfig: context.pdas.masterpoolConfig,
      masterpoolProgram: context.masterpool.programId,
      masterpoolProviderAccount: context.providerAccountPda,
      masterpoolProviderRewardAccount: context.providerRewardPda,
      masterpoolUserRewardAccount: context.userRewardPda,
      masterpoolReceiptSettlement: targets.settlementPda,
      masterpoolTreasuryUsdcVault: context.pdas.treasuryUsdcVault,
      masterpoolProviderPendingUsdcVault: context.pdas.providerPendingUsdcVault,
      usdcMint: context.testUsdcMint,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .instruction();

  const tx = new Transaction().add(ed25519Ix, submitIx);
  const signature = await context.provider.sendAndConfirm(tx, [context.payerKeypair]);

  return {
    signature,
    receiptHashHex: receiptHashHex(Uint8Array.from(compactArgs.receiptHash)),
  };
}

async function openChallengeCase(
  context: ChallengeContext,
  targets: ReturnType<typeof deriveChallengeCaseTargets>
): Promise<string> {
  return context.attestation.methods
    .openChallenge(CHALLENGE_TYPE_PAYLOAD_MISMATCH, CHALLENGE_EVIDENCE_HASH)
    .accounts({
      challenger: context.payerKeypair.publicKey,
      config: context.attestationConfigPda,
      receipt: targets.receiptPda,
      challenge: targets.challengePda,
      challengerClawToken: context.payerClawAta,
      masterpoolConfig: context.pdas.masterpoolConfig,
      masterpoolProgram: context.masterpool.programId,
      masterpoolReceiptSettlement: targets.settlementPda,
      masterpoolProviderAccount: context.providerAccountPda,
      masterpoolChallengeBondRecord: targets.challengeBondRecordPda,
      masterpoolChallengeBondVault: context.pdas.challengeBondVault,
      clawMint: context.clawMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([context.payerKeypair])
    .rpc();
}

async function resolveChallengeCase(
  context: ChallengeContext,
  targets: ReturnType<typeof deriveChallengeCaseTargets>,
  resolutionCode: number
): Promise<string> {
  return context.attestation.methods
    .resolveChallenge(resolutionCode)
    .accounts({
      challengeResolver: context.wallet.publicKey,
      config: context.attestationConfigPda,
      receipt: targets.receiptPda,
      challenge: targets.challengePda,
      masterpoolConfig: context.pdas.masterpoolConfig,
      masterpoolProgram: context.masterpool.programId,
      masterpoolReceiptSettlement: targets.settlementPda,
      masterpoolChallengeBondRecord: targets.challengeBondRecordPda,
      masterpoolProviderAccount: context.providerAccountPda,
      masterpoolProviderRewardAccount: context.providerRewardPda,
      masterpoolUserRewardAccount: context.userRewardPda,
      masterpoolChallengeBondVault: context.pdas.challengeBondVault,
      masterpoolRewardVault: context.pdas.rewardVault,
      masterpoolProviderPendingUsdcVault: context.pdas.providerPendingUsdcVault,
      challengerClawToken: context.payerClawAta,
      payerUsdcToken: context.payerUsdcAta,
      clawMint: context.clawMint,
      usdcMint: context.testUsdcMint,
      masterpoolPoolAuthority: context.pdas.poolAuthority,
      tokenProgram: TOKEN_PROGRAM_ID,
    } as any)
    .rpc();
}

async function timeoutRejectChallengeCase(
  context: ChallengeContext,
  targets: ReturnType<typeof deriveChallengeCaseTargets>
): Promise<string> {
  return context.attestation.methods
    .timeoutRejectChallenge()
    .accounts({
      authority: context.wallet.publicKey,
      config: context.attestationConfigPda,
      receipt: targets.receiptPda,
      challenge: targets.challengePda,
      masterpoolConfig: context.pdas.masterpoolConfig,
      masterpoolProgram: context.masterpool.programId,
      masterpoolReceiptSettlement: targets.settlementPda,
      masterpoolChallengeBondRecord: targets.challengeBondRecordPda,
      masterpoolProviderAccount: context.providerAccountPda,
      masterpoolProviderRewardAccount: context.providerRewardPda,
      masterpoolUserRewardAccount: context.userRewardPda,
      masterpoolChallengeBondVault: context.pdas.challengeBondVault,
      masterpoolRewardVault: context.pdas.rewardVault,
      masterpoolProviderPendingUsdcVault: context.pdas.providerPendingUsdcVault,
      challengerClawToken: context.payerClawAta,
      payerUsdcToken: context.payerUsdcAta,
      clawMint: context.clawMint,
      usdcMint: context.testUsdcMint,
      masterpoolPoolAuthority: context.pdas.poolAuthority,
      tokenProgram: TOKEN_PROGRAM_ID,
    } as any)
    .rpc();
}

async function finalizeReceiptWithTargets(args: {
  attestation: anchor.Program<any>;
  authority: PublicKey;
  attestationConfigPda: PublicKey;
  receiptPda: PublicKey;
  masterpool: anchor.Program<any>;
  pdas: ReturnType<typeof deriveMasterpoolPdas>;
  settlementPda: PublicKey;
  providerAccountPda: PublicKey;
  providerRewardPda: PublicKey;
  userRewardPda: PublicKey;
  providerDestinationUsdc: PublicKey;
  usdcMint: PublicKey;
}): Promise<string> {
  return args.attestation.methods
    .finalizeReceipt()
    .accounts({
      authority: args.authority,
      config: args.attestationConfigPda,
      receipt: args.receiptPda,
      masterpoolConfig: args.pdas.masterpoolConfig,
      masterpoolProgram: args.masterpool.programId,
      masterpoolReceiptSettlement: args.settlementPda,
      masterpoolProviderAccount: args.providerAccountPda,
      masterpoolProviderRewardAccount: args.providerRewardPda,
      masterpoolUserRewardAccount: args.userRewardPda,
      masterpoolProviderPendingUsdcVault: args.pdas.providerPendingUsdcVault,
      masterpoolProviderDestinationUsdc: args.providerDestinationUsdc,
      usdcMint: args.usdcMint,
      masterpoolPoolAuthority: args.pdas.poolAuthority,
      tokenProgram: TOKEN_PROGRAM_ID,
    } as any)
    .rpc();
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function requireNonEmptyString(value: string | undefined, label: string): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    throw new Error(`${label} is required`);
  }
  return trimmed;
}

function normalizeOptionalPath(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function normalizeChallengeCase(
  raw: RawPostSmokeChallengeCaseConfig,
  defaults: ChallengeReceiptDefaultsInput,
  label: string
): PostSmokeChallengeCaseConfig {
  const providerCode = firstDefinedNonEmptyString(
    [raw.providerCode, defaults.providerCode],
    "providerCode"
  );
  if (!providerCode) {
    throw new Error("providerCode is required when challenge cases are enabled");
  }

  const receiptChargeUiAmount = firstDefinedNonEmptyString(
    [raw.receiptChargeUiAmount, defaults.receiptChargeUiAmount],
    "receiptChargeUiAmount"
  );
  if (!receiptChargeUiAmount) {
    throw new Error(
      "receiptChargeUiAmount is required when challenge cases are enabled"
    );
  }

  return {
    requestNonce: requireNonEmptyString(raw.requestNonce, `${label}.requestNonce`),
    providerCode,
    proofId:
      firstDefinedNonEmptyString([raw.proofId, defaults.proofId], "proofId") ??
      DEFAULT_PROOF_ID,
    model:
      firstDefinedNonEmptyString([raw.model, defaults.model], "model") ??
      DEFAULT_MODEL,
    receiptChargeUiAmount,
    receiptChargeBaseUnits: toBaseUnits(receiptChargeUiAmount, 6),
    promptTokens: normalizeNonNegativeInteger(
      raw.promptTokens ?? defaults.promptTokens,
      DEFAULT_PROMPT_TOKENS,
      `${label}.promptTokens`
    ),
    completionTokens: normalizeNonNegativeInteger(
      raw.completionTokens ?? defaults.completionTokens,
      DEFAULT_COMPLETION_TOKENS,
      `${label}.completionTokens`
    ),
  };
}

function firstDefinedNonEmptyString(
  values: Array<string | undefined>,
  _label: string
): string | undefined {
  for (const value of values) {
    const trimmed = value?.trim();
    if (trimmed) {
      return trimmed;
    }
  }
  return undefined;
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

function requireValidPublicKeyString(
  value: string | undefined,
  label: string
): string {
  const trimmed = requireNonEmptyString(value, label);
  try {
    return new PublicKey(trimmed).toBase58();
  } catch {
    throw new Error(`${label} must be a valid public key`);
  }
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

async function fetchNullableProgramAccount<T>(
  connection: anchor.web3.Connection,
  accountClient: {
    fetch: (pubkey: PublicKey) => Promise<T>;
    fetchNullable?: (pubkey: PublicKey) => Promise<T | null>;
  },
  pubkey: PublicKey
): Promise<T | null> {
  if (accountClient.fetchNullable) {
    return accountClient.fetchNullable(pubkey);
  }
  if (!(await connection.getAccountInfo(pubkey))) {
    return null;
  }
  return accountClient.fetch(pubkey);
}

async function fetchMaybeProgramAccount<T>(
  connection: anchor.web3.Connection,
  accountClient: { fetch: (pubkey: PublicKey) => Promise<T> },
  pubkey: PublicKey
): Promise<T | null> {
  return fetchNullableProgramAccount(connection, accountClient, pubkey);
}

async function assertTokenBalanceAtLeast(
  connection: anchor.web3.Connection,
  tokenAccount: PublicKey,
  minimum: bigint,
  label: string
): Promise<void> {
  const current = (await getAccount(connection, tokenAccount)).amount;
  if (current < minimum) {
    throw new Error(
      `${label} is insufficient: ${current.toString()} < ${minimum.toString()}`
    );
  }
}

function toSafeNumber(value: unknown, label: string): number {
  const resolved =
    typeof value === "number"
      ? value
      : typeof value === "bigint"
        ? Number(value)
        : typeof value === "object" &&
            value !== null &&
            typeof (value as { toNumber?: unknown }).toNumber === "function"
          ? (value as { toNumber: () => number }).toNumber()
          : Number(value);
  if (!Number.isSafeInteger(resolved)) {
    throw new Error(`${label} must be a safe integer`);
  }
  return resolved;
}

function toBigIntString(value: unknown, label: string): string {
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { toString?: unknown }).toString === "function"
  ) {
    const rendered = (value as { toString: () => string }).toString();
    if (/^-?[0-9]+$/.test(rendered)) {
      return rendered;
    }
  }
  throw new Error(`${label} must be bigint-like`);
}

function decodeU64(value: anchor.BN | number): number {
  return typeof value === "number" ? value : value.toNumber();
}

async function currentUnixTime(
  connection: anchor.web3.Connection
): Promise<number> {
  const slot = await connection.getSlot("processed");
  const blockTime = await connection.getBlockTime(slot);
  return blockTime ?? Math.floor(Date.now() / 1000);
}

function assertBigIntEquals(actual: bigint, expected: bigint, label: string): void {
  if (actual !== expected) {
    throw new Error(`${label} mismatch: ${actual.toString()} != ${expected.toString()}`);
  }
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
  console.error(`[phase1-post-smoke] ${message}`);
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
