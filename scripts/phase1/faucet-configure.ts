import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { PublicKey, SystemProgram } from "@solana/web3.js";

import { DeploymentRecord, bn, deriveMasterpoolPdas, loadKeypair } from "./common";

export const DEFAULT_FAUCET_LIMITS = {
  maxClawPerClaim: bn(BigInt("10000000")),
  maxUsdcPerClaim: bn(BigInt("10000000")),
  maxClawPerWalletPerDay: bn(BigInt("50000000")),
  maxUsdcPerWalletPerDay: bn(BigInt("50000000")),
  maxClawGlobalPerDay: bn(BigInt("50000000000")),
  maxUsdcGlobalPerDay: bn(BigInt("50000000000")),
};

export interface FaucetConfigureArgs {
  deployment: string;
  adminKeypair: string;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
  initialize: boolean;
  enable: boolean;
  disable: boolean;
  updateLimits: boolean;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:faucet:configure --deployment <path> --admin-keypair <path> [--initialize] [--enable|--disable] [--update-limits] [--rpc-url <url>] [--masterpool-program-id <pubkey>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --admin-keypair",
    "",
    "Optional flags:",
    "  --initialize             Initializes faucet PDA accounts and vaults",
    "  --enable                 Enables faucet claims",
    "  --disable                Disables faucet claims",
    "  --update-limits          Applies default faucet limits",
    "  --rpc-url                Overrides deployment.rpcUrl",
    "  --masterpool-program-id  Overrides deployment.masterpoolProgramId",
  ].join("\n");
}

export function parseFaucetConfigureArgs(argv: string[]): FaucetConfigureArgs {
  const deployment = valueOf(argv, "--deployment");
  const adminKeypair = valueOf(argv, "--admin-keypair");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");
  const enable = argv.includes("--enable");
  const disable = argv.includes("--disable");
  const initialize = argv.includes("--initialize");
  const updateLimits = argv.includes("--update-limits");

  if (!deployment) throw new Error("deployment path is required");
  if (!adminKeypair) throw new Error("admin keypair path is required");
  if (enable && disable) throw new Error("choose either --enable or --disable");

  return {
    deployment,
    adminKeypair,
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId ? new PublicKey(masterpoolProgramId) : undefined,
    initialize,
    enable,
    disable,
    updateLimits,
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseFaucetConfigureArgs(argv);
  if (!args.initialize && !args.enable && !args.disable && !args.updateLimits) {
    throw new Error("choose at least one action: --initialize, --enable, --disable, or --update-limits");
  }

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

  const idl = JSON.parse(readFileSync("target/idl/clawfarm_masterpool.json", "utf8"));
  idl.address = masterpoolProgramId.toBase58();
  const masterpool = new anchor.Program(idl as anchor.Idl, provider);
  const pdas = deriveMasterpoolPdas(masterpoolProgramId);
  const clawMint = new PublicKey(deployment.clawMint);
  const testUsdcMint = new PublicKey(deployment.testUsdcMint);

  let signature: string | undefined;
  if (args.initialize) {
    signature = await masterpool.methods
      .initializeFaucet()
      .accounts({
        config: pdas.masterpoolConfig,
        faucetConfig: pdas.faucetConfig,
        faucetGlobalState: pdas.faucetGlobal,
        faucetClawVault: pdas.faucetClawVault,
        faucetUsdcVault: pdas.faucetUsdcVault,
        clawMint,
        usdcMint: testUsdcMint,
        poolAuthority: pdas.poolAuthority,
        adminAuthority: admin.publicKey,
        payer: admin.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      } as any)
      .signers([admin])
      .rpc();
  }

  if (args.updateLimits) {
    signature = await masterpool.methods
      .updateFaucetLimits(DEFAULT_FAUCET_LIMITS)
      .accounts({
        config: pdas.masterpoolConfig,
        faucetConfig: pdas.faucetConfig,
        adminAuthority: admin.publicKey,
      } as any)
      .signers([admin])
      .rpc();
  }

  let enabled: boolean | undefined;
  if (args.enable || args.disable) {
    enabled = args.enable;
    signature = await masterpool.methods
      .setFaucetEnabled(enabled)
      .accounts({
        config: pdas.masterpoolConfig,
        faucetConfig: pdas.faucetConfig,
        adminAuthority: admin.publicKey,
      } as any)
      .signers([admin])
      .rpc();
  }

  if (enabled === undefined) {
    const faucetConfig = await (masterpool.account as any).faucetConfig.fetchNullable(
      pdas.faucetConfig
    );
    enabled = faucetConfig?.enabled;
  }

  console.log(
    JSON.stringify(
      {
        signature,
        faucetConfig: pdas.faucetConfig.toBase58(),
        faucetGlobal: pdas.faucetGlobal.toBase58(),
        faucetClawVault: pdas.faucetClawVault.toBase58(),
        faucetUsdcVault: pdas.faucetUsdcVault.toBase58(),
        enabled,
      },
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
