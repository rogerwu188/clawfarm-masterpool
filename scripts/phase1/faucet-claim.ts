import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { getOrCreateAssociatedTokenAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { PublicKey, SystemProgram } from "@solana/web3.js";

import { DeploymentRecord, bn, deriveMasterpoolPdas, loadKeypair, toBaseUnits } from "./common";

export interface FaucetClaimArgs {
  deployment: string;
  userPublicKey: PublicKey;
  feePayerKeypair: string;
  clawAmountBaseUnits: bigint;
  usdcAmountBaseUnits: bigint;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:faucet:claim --deployment <path> --user-public-key <pubkey> --fee-payer-keypair <path> [--claw-amount <ui-amount>] [--usdc-amount <ui-amount>] [--rpc-url <url>] [--masterpool-program-id <pubkey>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --user-public-key     Recipient wallet public key; this key does not sign",
    "  --fee-payer-keypair   Keypair that signs the transaction and pays fees/ATA rent",
    "",
    "Claim amount flags:",
    "  --claw-amount  UI CLAW amount, converted with 6 decimals",
    "  --usdc-amount  UI Test USDC amount, converted with 6 decimals",
    "",
    "Optional flags:",
    "  --rpc-url                Overrides deployment.rpcUrl",
    "  --masterpool-program-id  Overrides deployment.masterpoolProgramId",
  ].join("\n");
}


export function parseFaucetClaimArgs(argv: string[]): FaucetClaimArgs {
  const deployment = valueOf(argv, "--deployment");
  const userPublicKey = valueOf(argv, "--user-public-key");
  const feePayerKeypair = valueOf(argv, "--fee-payer-keypair");
  const clawAmount = valueOf(argv, "--claw-amount");
  const usdcAmount = valueOf(argv, "--usdc-amount");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");

  if (!deployment) throw new Error("deployment path is required");
  if (!userPublicKey) throw new Error("user public key is required");
  if (!feePayerKeypair) throw new Error("fee payer keypair path is required");
  if (!clawAmount && !usdcAmount) throw new Error("at least one claim amount is required");

  const clawAmountBaseUnits = clawAmount ? toBaseUnits(clawAmount, 6) : BigInt(0);
  const usdcAmountBaseUnits = usdcAmount ? toBaseUnits(usdcAmount, 6) : BigInt(0);
  if (clawAmountBaseUnits === BigInt(0) && usdcAmountBaseUnits === BigInt(0)) {
    throw new Error("at least one claim amount must be positive");
  }

  return {
    deployment,
    userPublicKey: new PublicKey(userPublicKey),
    feePayerKeypair,
    clawAmountBaseUnits,
    usdcAmountBaseUnits,
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId ? new PublicKey(masterpoolProgramId) : undefined,
  };
}



function bigintString(value: { toString(): string } | bigint): string {
  return value.toString();
}

function subtractFloorZero(left: { toString(): string }, right: { toString(): string }): string {
  const result = BigInt(left.toString()) - BigInt(right.toString());
  return (result > BigInt(0) ? result : BigInt(0)).toString();
}

export function buildFaucetClaimReport(input: {
  signature: string;
  userPublicKey: string;
  feePayer: string;
  sourceAuthority: string;
  userFaucetState: string;
  userClawToken: string;
  userUsdcToken: string;
  clawAmountBaseUnits: bigint;
  usdcAmountBaseUnits: bigint;
  faucetConfig: {
    maxClawPerWalletPerDay: { toString(): string };
    maxUsdcPerWalletPerDay: { toString(): string };
  };
  userState: {
    currentDayIndex: { toString(): string };
    clawClaimedToday: { toString(): string };
    usdcClaimedToday: { toString(): string };
  };
}) {
  return {
    signature: input.signature,
    recipient: input.userPublicKey,
    userPublicKey: input.userPublicKey,
    feePayer: input.feePayer,
    sourceAuthority: input.sourceAuthority,
    userFaucetState: input.userFaucetState,
    userClawToken: input.userClawToken,
    userUsdcToken: input.userUsdcToken,
    clawAmount: input.clawAmountBaseUnits.toString(),
    usdcAmount: input.usdcAmountBaseUnits.toString(),
    walletDailyQuota: {
      currentDayIndex: bigintString(input.userState.currentDayIndex),
      claw: {
        limit: bigintString(input.faucetConfig.maxClawPerWalletPerDay),
        claimedToday: bigintString(input.userState.clawClaimedToday),
        remaining: subtractFloorZero(
          input.faucetConfig.maxClawPerWalletPerDay,
          input.userState.clawClaimedToday
        ),
      },
      usdc: {
        limit: bigintString(input.faucetConfig.maxUsdcPerWalletPerDay),
        claimedToday: bigintString(input.userState.usdcClaimedToday),
        remaining: subtractFloorZero(
          input.faucetConfig.maxUsdcPerWalletPerDay,
          input.userState.usdcClaimedToday
        ),
      },
    },
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseFaucetClaimArgs(argv);
  const deployment = JSON.parse(
    readFileSync(args.deployment, "utf8")
  ) as DeploymentRecord;
  const rpcUrl = args.rpcUrl ?? deployment.rpcUrl;
  const masterpoolProgramId =
    args.masterpoolProgramId ?? new PublicKey(deployment.masterpoolProgramId);
  const feePayer = await loadKeypair(args.feePayerKeypair);
  const userPublicKey = args.userPublicKey;
  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(feePayer);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  const idl = JSON.parse(readFileSync("target/idl/clawfarm_masterpool.json", "utf8"));
  idl.address = masterpoolProgramId.toBase58();
  const masterpool = new anchor.Program(idl as anchor.Idl, provider);
  const pdas = deriveMasterpoolPdas(masterpoolProgramId);
  const clawMint = new PublicKey(deployment.clawMint);
  const usdcMint = new PublicKey(deployment.testUsdcMint);

  const [userFaucetState] = PublicKey.findProgramAddressSync(
    [Buffer.from("faucet_user"), userPublicKey.toBuffer()],
    masterpoolProgramId
  );
  const userClawToken = await getOrCreateAssociatedTokenAccount(
    connection,
    feePayer,
    clawMint,
    userPublicKey
  );
  const userUsdcToken = await getOrCreateAssociatedTokenAccount(
    connection,
    feePayer,
    usdcMint,
    userPublicKey
  );

  const signature = await masterpool.methods
    .claimFaucet({
      clawAmount: bn(args.clawAmountBaseUnits),
      usdcAmount: bn(args.usdcAmountBaseUnits),
    })
    .accounts({
      config: pdas.masterpoolConfig,
      faucetConfig: pdas.faucetConfig,
      faucetGlobalState: pdas.faucetGlobal,
      faucetUserState: userFaucetState,
      faucetClawVault: pdas.faucetClawVault,
      faucetUsdcVault: pdas.faucetUsdcVault,
      userClawToken: userClawToken.address,
      userUsdcToken: userUsdcToken.address,
      clawMint,
      usdcMint,
      poolAuthority: pdas.poolAuthority,
      user: userPublicKey,
      payer: feePayer.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([feePayer])
    .rpc();

  const [faucetConfig, userState] = await Promise.all([
    (masterpool.account as any).faucetConfig.fetch(pdas.faucetConfig),
    (masterpool.account as any).faucetUserState.fetch(userFaucetState),
  ]);

  console.log(
    JSON.stringify(
      buildFaucetClaimReport({
        signature,
        userPublicKey: userPublicKey.toBase58(),
        feePayer: feePayer.publicKey.toBase58(),
        sourceAuthority: pdas.poolAuthority.toBase58(),
        userFaucetState: userFaucetState.toBase58(),
        userClawToken: userClawToken.address.toBase58(),
        userUsdcToken: userUsdcToken.address.toBase58(),
        clawAmountBaseUnits: args.clawAmountBaseUnits,
        usdcAmountBaseUnits: args.usdcAmountBaseUnits,
        faucetConfig,
        userState,
      }),
      null,
      2
    )
  );
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
