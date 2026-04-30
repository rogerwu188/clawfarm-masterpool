import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { PublicKey } from "@solana/web3.js";

import {
  DeploymentRecord,
  bn,
  deriveMasterpoolPdas,
  loadKeypair,
} from "./common";

export interface UpdateConfigArgs {
  deployment: string;
  adminKeypair: string;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}

export const PHASE1_70_30_PARAMS = {
  exchangeRateClawPerUsdcE6: bn(BigInt("1000000")),
  providerStakeUsdc: bn(BigInt("100000000")),
  providerUsdcShareBps: 700,
  treasuryUsdcShareBps: 300,
  userClawShareBps: 300,
  providerClawShareBps: 700,
  lockDays: 180,
  providerSlashClawAmount: bn(BigInt("30000000")),
  challengerRewardBps: 700,
  burnBps: 300,
  challengeBondClawAmount: bn(BigInt("2000000")),
};

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:update-config --deployment <path> --admin-keypair <path> [--rpc-url <url>] [--masterpool-program-id <pubkey>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --admin-keypair",
    "",
    "Optional flags:",
    "  --rpc-url                Overrides deployment.rpcUrl",
    "  --masterpool-program-id  Overrides deployment.masterpoolProgramId",
  ].join("\n");
}

export function parseUpdateConfigArgs(argv: string[]): UpdateConfigArgs {
  const deployment = valueOf(argv, "--deployment");
  const adminKeypair = valueOf(argv, "--admin-keypair");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");

  if (!deployment) throw new Error("deployment path is required");
  if (!adminKeypair) throw new Error("admin keypair path is required");

  return {
    deployment,
    adminKeypair,
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId
      ? new PublicKey(masterpoolProgramId)
      : undefined,
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseUpdateConfigArgs(argv);
  const deployment = JSON.parse(
    readFileSync(args.deployment, "utf8")
  ) as DeploymentRecord;
  const rpcUrl = args.rpcUrl ?? deployment.rpcUrl;
  const masterpoolProgramId =
    args.masterpoolProgramId ?? new PublicKey(deployment.masterpoolProgramId);
  const admin = await loadKeypair(args.adminKeypair);

  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  const masterpoolIdl = JSON.parse(
    readFileSync("target/idl/clawfarm_masterpool.json", "utf8")
  );
  masterpoolIdl.address = masterpoolProgramId.toBase58();
  const masterpool = new anchor.Program(masterpoolIdl as anchor.Idl, provider);
  const pdas = deriveMasterpoolPdas(masterpoolProgramId);

  const signature = await masterpool.methods
    .updateConfig(PHASE1_70_30_PARAMS)
    .accounts({
      config: pdas.masterpoolConfig,
      adminAuthority: admin.publicKey,
    } as any)
    .signers([admin])
    .rpc();

  const config = await (masterpool.account as any).globalConfig.fetch(
    pdas.masterpoolConfig
  );
  console.log(
    JSON.stringify(
      {
        signature,
        masterpoolProgramId: masterpoolProgramId.toBase58(),
        config: pdas.masterpoolConfig.toBase58(),
        providerUsdcShareBps: config.providerUsdcShareBps,
        treasuryUsdcShareBps: config.treasuryUsdcShareBps,
      },
      null,
      2
    )
  );
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
