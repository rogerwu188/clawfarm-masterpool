import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { getAccount } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";

import { DeploymentRecord, deriveMasterpoolPdas } from "./common";

export interface FaucetStatusArgs {
  deployment: string;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:faucet:status --deployment <path> [--rpc-url <url>] [--masterpool-program-id <pubkey>]",
    "",
    "Required flags:",
    "  --deployment",
    "",
    "Optional flags:",
    "  --rpc-url                Overrides deployment.rpcUrl",
    "  --masterpool-program-id  Overrides deployment.masterpoolProgramId",
  ].join("\n");
}

export function parseFaucetStatusArgs(argv: string[]): FaucetStatusArgs {
  const deployment = valueOf(argv, "--deployment");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");

  if (!deployment) throw new Error("deployment path is required");

  return {
    deployment,
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId ? new PublicKey(masterpoolProgramId) : undefined,
  };
}

function stringifyAccount(value: any): any {
  if (anchor.BN.isBN(value)) return value.toString();
  if (value instanceof PublicKey) return value.toBase58();
  if (Array.isArray(value)) return value.map(stringifyAccount);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, nested]) => [key, stringifyAccount(nested)])
    );
  }
  return value;
}


interface TokenAccountSummaryInput {
  mint: PublicKey;
  owner: PublicKey;
  amount: bigint;
}

interface FaucetStatusReportInput {
  currentDayIndex: number;
  pdas: {
    faucetConfig: PublicKey;
    faucetGlobal: PublicKey;
    faucetClawVault: PublicKey;
    faucetUsdcVault: PublicKey;
  };
  faucetConfig: any;
  faucetGlobalState: any;
  rewardVault: TokenAccountSummaryInput;
  clawVault: TokenAccountSummaryInput;
  usdcVault: TokenAccountSummaryInput;
}

function tokenAccountSummary(account: TokenAccountSummaryInput) {
  return {
    mint: account.mint.toBase58(),
    owner: account.owner.toBase58(),
    amount: account.amount.toString(),
  };
}

export function buildFaucetStatusReport(input: FaucetStatusReportInput) {
  return {
    initialized: true,
    currentDayIndex: input.currentDayIndex,
    faucetConfig: input.pdas.faucetConfig.toBase58(),
    faucetGlobal: input.pdas.faucetGlobal.toBase58(),
    faucetClawVault: input.pdas.faucetClawVault.toBase58(),
    faucetUsdcVault: input.pdas.faucetUsdcVault.toBase58(),
    config: stringifyAccount(input.faucetConfig),
    globalState: stringifyAccount(input.faucetGlobalState),
    vaults: {
      rewardClaw: tokenAccountSummary(input.rewardVault),
      faucetClaw: tokenAccountSummary(input.clawVault),
      faucetUsdc: tokenAccountSummary(input.usdcVault),
    },
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseFaucetStatusArgs(argv);
  const deployment = JSON.parse(
    readFileSync(args.deployment, "utf8")
  ) as DeploymentRecord;
  const rpcUrl = args.rpcUrl ?? deployment.rpcUrl;
  const masterpoolProgramId =
    args.masterpoolProgramId ?? new PublicKey(deployment.masterpoolProgramId);
  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(anchor.web3.Keypair.generate()),
    { commitment: "confirmed" }
  );

  const idl = JSON.parse(readFileSync("target/idl/clawfarm_masterpool.json", "utf8"));
  idl.address = masterpoolProgramId.toBase58();
  const masterpool = new anchor.Program(idl as anchor.Idl, provider);
  const pdas = deriveMasterpoolPdas(masterpoolProgramId);
  const faucetConfig = await (masterpool.account as any).faucetConfig.fetchNullable(
    pdas.faucetConfig
  );

  if (!faucetConfig) {
    console.log(JSON.stringify({ initialized: false }, null, 2));
    return;
  }

  const [faucetGlobalState, rewardVault, clawVault, usdcVault] = await Promise.all([
    (masterpool.account as any).faucetGlobalState.fetch(pdas.faucetGlobal),
    getAccount(connection, pdas.rewardVault),
    getAccount(connection, pdas.faucetClawVault),
    getAccount(connection, pdas.faucetUsdcVault),
  ]);
  const currentDayIndex = Math.floor(Date.now() / 1000 / 86400);

  console.log(
    JSON.stringify(
      buildFaucetStatusReport({
        currentDayIndex,
        pdas,
        faucetConfig,
        faucetGlobalState,
        rewardVault,
        clawVault,
        usdcVault,
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
