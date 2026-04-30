import { readFileSync } from "fs";
import {
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { Connection, PublicKey } from "@solana/web3.js";

import { DeploymentRecord, loadKeypair, toBaseUnits } from "./common";

export interface MintTestUsdcArgs {
  deployment: string;
  operatorKeypair: string;
  recipient: PublicKey;
  amountBaseUnits: bigint;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

function usage(): string {
  return [
    "Usage: yarn phase1:mint:test-usdc --deployment <path> --operator-keypair <path> --recipient <pubkey> --amount <ui-amount>",
    "",
    "Required flags:",
    "  --deployment",
    "  --operator-keypair",
    "  --recipient",
    "  --amount",
  ].join("\n");
}

export function parseMintTestUsdcArgs(argv: string[]): MintTestUsdcArgs {
  const deployment = valueOf(argv, "--deployment");
  const operatorKeypair = valueOf(argv, "--operator-keypair");
  const recipient = valueOf(argv, "--recipient");
  const amount = valueOf(argv, "--amount");

  if (!deployment) throw new Error("deployment record is required");
  if (!operatorKeypair) throw new Error("operator keypair is required");
  if (!recipient) throw new Error("recipient is required");
  if (!amount) throw new Error("amount is required");

  return {
    deployment,
    operatorKeypair,
    recipient: new PublicKey(recipient),
    amountBaseUnits: toBaseUnits(amount, 6),
  };
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--help")) {
    console.log(usage());
    return;
  }

  const args = parseMintTestUsdcArgs(argv);
  const record = JSON.parse(
    readFileSync(args.deployment, "utf8")
  ) as DeploymentRecord;
  const operator = await loadKeypair(args.operatorKeypair);
  const connection = new Connection(record.rpcUrl, "confirmed");
  const mint = new PublicKey(record.testUsdcMint);

  const recipientAta = await getOrCreateAssociatedTokenAccount(
    connection,
    operator,
    mint,
    args.recipient
  );

  const signature = await mintTo(
    connection,
    operator,
    mint,
    recipientAta.address,
    operator,
    BigInt(args.amountBaseUnits.toString())
  );

  console.log(
    JSON.stringify(
      {
        signature,
        recipient: args.recipient.toBase58(),
        recipientAta: recipientAta.address.toBase58(),
        amount: args.amountBaseUnits.toString(),
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
