import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import {
  AuthorityType,
  TOKEN_PROGRAM_ID,
  createMint,
  setAuthority,
} from "@solana/spl-token";
import { PublicKey, SystemProgram } from "@solana/web3.js";

import {
  bn,
  deriveAttestationConfig,
  deriveMasterpoolPdas,
  deriveProgramDataAddress,
  loadKeypair,
  writeDeploymentRecord,
} from "./common";

export interface BootstrapArgs {
  cluster: string;
  rpcUrl: string;
  adminKeypair: string;
  testUsdcOperatorKeypair: string;
  masterpoolProgramId: PublicKey;
  attestationProgramId: PublicKey;
  out: string;
}

const DEFAULT_PHASE1_PARAMS = {
  exchangeRateClawPerUsdcE6: bn(1_000_000n),
  providerStakeUsdc: bn(100_000_000n),
  providerUsdcShareBps: 700,
  treasuryUsdcShareBps: 300,
  userClawShareBps: 300,
  providerClawShareBps: 700,
  lockDays: 180,
  providerSlashClawAmount: bn(30_000_000n),
  challengerRewardBps: 700,
  burnBps: 300,
  challengeBondClawAmount: bn(2_000_000n),
};

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:bootstrap:testnet --cluster <cluster> --rpc-url <url> --admin-keypair <path> --test-usdc-operator-keypair <path> --masterpool-program-id <pubkey> --attestation-program-id <pubkey> [--out <path>]",
    "",
    "Required flags:",
    "  --cluster",
    "  --rpc-url",
    "  --admin-keypair",
    "  --test-usdc-operator-keypair",
    "  --masterpool-program-id",
    "  --attestation-program-id",
    "",
    "Optional flags:",
    "  --out deployments/devnet-phase1.json",
  ].join("\n");
}

export function parseBootstrapArgs(argv: string[]): BootstrapArgs {
  const cluster = valueOf(argv, "--cluster");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const adminKeypair = valueOf(argv, "--admin-keypair");
  const testUsdcOperatorKeypair = valueOf(argv, "--test-usdc-operator-keypair");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");
  const attestationProgramId = valueOf(argv, "--attestation-program-id");
  const out = valueOf(argv, "--out") ?? "deployments/devnet-phase1.json";

  if (!cluster || !rpcUrl || !adminKeypair || !testUsdcOperatorKeypair) {
    throw new Error("missing required bootstrap arguments");
  }
  if (!masterpoolProgramId || !attestationProgramId) {
    throw new Error("program ids are required");
  }
  if (adminKeypair === testUsdcOperatorKeypair) {
    throw new Error("admin and test usdc operator keypairs must differ");
  }

  return {
    cluster,
    rpcUrl,
    adminKeypair,
    testUsdcOperatorKeypair,
    masterpoolProgramId: new PublicKey(masterpoolProgramId),
    attestationProgramId: new PublicKey(attestationProgramId),
    out,
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseBootstrapArgs(argv);
  const admin = await loadKeypair(args.adminKeypair);
  const operator = await loadKeypair(args.testUsdcOperatorKeypair);
  const connection = new anchor.web3.Connection(args.rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  const masterpoolIdl = JSON.parse(
    readFileSync("target/idl/clawfarm_masterpool.json", "utf8")
  );
  const attestationIdl = JSON.parse(
    readFileSync("target/idl/clawfarm_attestation.json", "utf8")
  );

  masterpoolIdl.address = args.masterpoolProgramId.toBase58();
  attestationIdl.address = args.attestationProgramId.toBase58();

  const masterpool = new anchor.Program(masterpoolIdl as anchor.Idl, provider);
  const attestation = new anchor.Program(attestationIdl as anchor.Idl, provider);

  const pdas = deriveMasterpoolPdas(args.masterpoolProgramId);
  const attestationConfig = deriveAttestationConfig(args.attestationProgramId);
  const masterpoolProgramData = deriveProgramDataAddress(
    args.masterpoolProgramId
  );
  const attestationProgramData = deriveProgramDataAddress(
    args.attestationProgramId
  );

  const clawMint = await createMint(
    connection,
    admin,
    admin.publicKey,
    admin.publicKey,
    6
  );
  const testUsdcMint = await createMint(
    connection,
    admin,
    operator.publicKey,
    null,
    6
  );

  await setAuthority(
    connection,
    admin,
    clawMint,
    admin,
    AuthorityType.MintTokens,
    pdas.poolAuthority
  );
  await setAuthority(
    connection,
    admin,
    clawMint,
    admin,
    AuthorityType.FreezeAccount,
    pdas.poolAuthority
  );

  await masterpool.methods
    .initializeMasterpool(DEFAULT_PHASE1_PARAMS)
    .accounts({
      config: pdas.masterpoolConfig,
      rewardVault: pdas.rewardVault,
      challengeBondVault: pdas.challengeBondVault,
      treasuryUsdcVault: pdas.treasuryUsdcVault,
      providerStakeUsdcVault: pdas.providerStakeUsdcVault,
      providerPendingUsdcVault: pdas.providerPendingUsdcVault,
      clawMint,
      usdcMint: testUsdcMint,
      attestationProgram: args.attestationProgramId,
      selfProgram: args.masterpoolProgramId,
      selfProgramData: masterpoolProgramData,
      poolAuthority: pdas.poolAuthority,
      initializer: admin.publicKey,
      admin: admin.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([admin])
    .rpc();

  await masterpool.methods
    .mintGenesisSupply()
    .accounts({
      config: pdas.masterpoolConfig,
      adminAuthority: admin.publicKey,
      rewardVault: pdas.rewardVault,
      clawMint,
      poolAuthority: pdas.poolAuthority,
      tokenProgram: TOKEN_PROGRAM_ID,
    } as any)
    .signers([admin])
    .rpc();

  await attestation.methods
    .initializeConfig(
      admin.publicKey,
      admin.publicKey,
      admin.publicKey,
      args.masterpoolProgramId,
      new anchor.BN(86_400),
      new anchor.BN(86_400)
    )
    .accounts({
      initializer: admin.publicKey,
      config: attestationConfig,
      selfProgram: args.attestationProgramId,
      selfProgramData: attestationProgramData,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([admin])
    .rpc();

  await writeDeploymentRecord(args.out, {
    cluster: args.cluster,
    rpcUrl: args.rpcUrl,
    masterpoolProgramId: args.masterpoolProgramId.toBase58(),
    attestationProgramId: args.attestationProgramId.toBase58(),
    clawMint: clawMint.toBase58(),
    testUsdcMint: testUsdcMint.toBase58(),
    poolAuthority: pdas.poolAuthority.toBase58(),
    masterpoolConfig: pdas.masterpoolConfig.toBase58(),
    attestationConfig: attestationConfig.toBase58(),
    rewardVault: pdas.rewardVault.toBase58(),
    challengeBondVault: pdas.challengeBondVault.toBase58(),
    treasuryUsdcVault: pdas.treasuryUsdcVault.toBase58(),
    providerStakeUsdcVault: pdas.providerStakeUsdcVault.toBase58(),
    providerPendingUsdcVault: pdas.providerPendingUsdcVault.toBase58(),
    adminAuthority: admin.publicKey.toBase58(),
    testUsdcOperator: operator.publicKey.toBase58(),
    createdAt: new Date().toISOString(),
  });
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
