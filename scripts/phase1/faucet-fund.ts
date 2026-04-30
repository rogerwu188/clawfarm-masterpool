import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { getAssociatedTokenAddressSync, mintTo, transfer } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";

import { DeploymentRecord, bn, deriveMasterpoolPdas, loadKeypair, toBaseUnits } from "./common";

export type FaucetFundToken = "claw" | "usdc";

export interface FaucetFundArgs {
  deployment: string;
  token: FaucetFundToken;
  amountBaseUnits: bigint;
  adminKeypair?: string;
  fundingKeypair?: string;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:faucet:fund --deployment <path> --token <claw|usdc> --amount <ui-amount> [--admin-keypair <path>] [--funding-keypair <path>] [--rpc-url <url>] [--masterpool-program-id <pubkey>]",
    "",
    "Required flags:",
    "  --deployment",
    "  --token claw|usdc",
    "  --amount",
    "",
    "Funding authority:",
    "  --admin-keypair    Required for --token claw; moves CLAW from reward_vault to faucet_claw_vault",
    "  --funding-keypair  Required for --token usdc; operator mints or wallet transfers Test USDC",
    "",
    "Optional flags:",
    "  --rpc-url                Overrides deployment.rpcUrl",
    "  --masterpool-program-id  Overrides deployment.masterpoolProgramId",
  ].join("\n");
}

export function parseFaucetFundArgs(argv: string[]): FaucetFundArgs {
  const deployment = valueOf(argv, "--deployment");
  const adminKeypair = valueOf(argv, "--admin-keypair");
  const fundingKeypair = valueOf(argv, "--funding-keypair");
  const token = valueOf(argv, "--token");
  const amount = valueOf(argv, "--amount");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");

  if (!deployment) throw new Error("deployment path is required");
  if (token !== "claw" && token !== "usdc") throw new Error("token must be claw or usdc");
  if (token === "claw" && !adminKeypair) {
    throw new Error("admin keypair path is required for CLAW funding");
  }
  if (token === "usdc" && !fundingKeypair) {
    throw new Error("funding keypair path is required for USDC funding");
  }
  if (!amount) throw new Error("amount is required");

  return {
    deployment,
    adminKeypair,
    fundingKeypair,
    token,
    amountBaseUnits: toBaseUnits(amount, 6),
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId ? new PublicKey(masterpoolProgramId) : undefined,
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseFaucetFundArgs(argv);
  const deployment = JSON.parse(
    readFileSync(args.deployment, "utf8")
  ) as DeploymentRecord;
  const rpcUrl = args.rpcUrl ?? deployment.rpcUrl;
  const masterpoolProgramId =
    args.masterpoolProgramId ?? new PublicKey(deployment.masterpoolProgramId);
  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const pdas = deriveMasterpoolPdas(masterpoolProgramId);

  let signature: string;
  let destinationVault: PublicKey;
  if (args.token === "claw") {
    const admin = await loadKeypair(args.adminKeypair!);
    const wallet = new anchor.Wallet(admin);
    const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
    anchor.setProvider(provider);
    const idl = JSON.parse(readFileSync("target/idl/clawfarm_masterpool.json", "utf8"));
    idl.address = masterpoolProgramId.toBase58();
    const masterpool = new anchor.Program(idl as anchor.Idl, provider);
    destinationVault = pdas.faucetClawVault;
    signature = await masterpool.methods
      .fundFaucetClaw(bn(args.amountBaseUnits))
      .accounts({
        config: pdas.masterpoolConfig,
        faucetConfig: pdas.faucetConfig,
        rewardVault: pdas.rewardVault,
        faucetClawVault: pdas.faucetClawVault,
        clawMint: new PublicKey(deployment.clawMint),
        poolAuthority: pdas.poolAuthority,
        adminAuthority: admin.publicKey,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      } as any)
      .signers([admin])
      .rpc();
  } else {
    const funder = await loadKeypair(args.fundingKeypair!);
    const mint = new PublicKey(deployment.testUsdcMint);
    destinationVault = pdas.faucetUsdcVault;
    if (
      deployment.testUsdcOperator &&
      funder.publicKey.equals(new PublicKey(deployment.testUsdcOperator))
    ) {
      signature = await mintTo(
        connection,
        funder,
        mint,
        destinationVault,
        funder,
        BigInt(args.amountBaseUnits.toString())
      );
    } else {
      const sourceAta = getAssociatedTokenAddressSync(mint, funder.publicKey);
      signature = await transfer(
        connection,
        funder,
        sourceAta,
        destinationVault,
        funder,
        BigInt(args.amountBaseUnits.toString())
      );
    }
  }

  console.log(
    JSON.stringify(
      {
        signature,
        token: args.token,
        amount: args.amountBaseUnits.toString(),
        destinationVault: destinationVault.toBase58(),
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
