import * as anchor from "@coral-xyz/anchor";
import { promises as fs } from "fs";
import path from "path";
import { Keypair, PublicKey } from "@solana/web3.js";

export interface DeploymentRecord {
  cluster: string;
  rpcUrl: string;
  masterpoolProgramId: string;
  attestationProgramId: string;
  clawMint: string;
  testUsdcMint: string;
  poolAuthority: string;
  masterpoolConfig: string;
  attestationConfig: string;
  rewardVault: string;
  challengeBondVault: string;
  treasuryUsdcVault: string;
  providerStakeUsdcVault: string;
  providerPendingUsdcVault: string;
  faucetConfig?: string;
  faucetGlobal?: string;
  faucetClawVault?: string;
  faucetUsdcVault?: string;
  adminAuthority: string;
  testUsdcOperator: string;
  createdAt: string;
}

const CONFIG_SEED = Buffer.from("config");
const POOL_AUTHORITY_SEED = Buffer.from("pool_authority");
const REWARD_VAULT_SEED = Buffer.from("reward_vault");
const CHALLENGE_BOND_VAULT_SEED = Buffer.from("challenge_bond_vault");
const TREASURY_USDC_VAULT_SEED = Buffer.from("treasury_usdc_vault");
const PROVIDER_STAKE_USDC_VAULT_SEED = Buffer.from("provider_stake_usdc_vault");
const PROVIDER_PENDING_USDC_VAULT_SEED = Buffer.from("provider_pending_usdc_vault");
const FAUCET_CONFIG_SEED = Buffer.from("faucet_config");
const FAUCET_GLOBAL_SEED = Buffer.from("faucet_global");
const FAUCET_CLAW_VAULT_SEED = Buffer.from("faucet_claw_vault");
const FAUCET_USDC_VAULT_SEED = Buffer.from("faucet_usdc_vault");
const UPGRADEABLE_LOADER_PROGRAM_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111"
);

export function deriveMasterpoolPdas(programId: PublicKey) {
  const [masterpoolConfig] = PublicKey.findProgramAddressSync(
    [CONFIG_SEED],
    programId
  );
  const [poolAuthority] = PublicKey.findProgramAddressSync(
    [POOL_AUTHORITY_SEED],
    programId
  );
  const [rewardVault] = PublicKey.findProgramAddressSync(
    [REWARD_VAULT_SEED],
    programId
  );
  const [challengeBondVault] = PublicKey.findProgramAddressSync(
    [CHALLENGE_BOND_VAULT_SEED],
    programId
  );
  const [treasuryUsdcVault] = PublicKey.findProgramAddressSync(
    [TREASURY_USDC_VAULT_SEED],
    programId
  );
  const [providerStakeUsdcVault] = PublicKey.findProgramAddressSync(
    [PROVIDER_STAKE_USDC_VAULT_SEED],
    programId
  );
  const [providerPendingUsdcVault] = PublicKey.findProgramAddressSync(
    [PROVIDER_PENDING_USDC_VAULT_SEED],
    programId
  );
  const [faucetConfig] = PublicKey.findProgramAddressSync(
    [FAUCET_CONFIG_SEED],
    programId
  );
  const [faucetGlobal] = PublicKey.findProgramAddressSync(
    [FAUCET_GLOBAL_SEED],
    programId
  );
  const [faucetClawVault] = PublicKey.findProgramAddressSync(
    [FAUCET_CLAW_VAULT_SEED],
    programId
  );
  const [faucetUsdcVault] = PublicKey.findProgramAddressSync(
    [FAUCET_USDC_VAULT_SEED],
    programId
  );

  return {
    masterpoolConfig,
    poolAuthority,
    rewardVault,
    challengeBondVault,
    treasuryUsdcVault,
    providerStakeUsdcVault,
    providerPendingUsdcVault,
    faucetConfig,
    faucetGlobal,
    faucetClawVault,
    faucetUsdcVault,
  };
}

export function deriveAttestationConfig(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], programId)[0];
}

export function deriveProgramDataAddress(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [programId.toBuffer()],
    UPGRADEABLE_LOADER_PROGRAM_ID
  )[0];
}

export function toBaseUnits(input: string, decimals: number): bigint {
  if (!/^[0-9]+(\.[0-9]+)?$/.test(input)) {
    throw new Error(`invalid token amount: ${input}`);
  }

  const [whole, fractional = ""] = input.split(".");
  if (fractional.length > decimals) {
    throw new Error("too many decimal places");
  }

  const paddedFractional = fractional.padEnd(decimals, "0");
  return BigInt(`${whole}${paddedFractional}`);
}

export async function loadKeypair(keypairPath: string): Promise<Keypair> {
  const file = await fs.readFile(keypairPath, "utf8");
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(file)));
}

export async function writeDeploymentRecord(
  outputPath: string,
  record: DeploymentRecord
): Promise<void> {
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(record, null, 2)}\n`, "utf8");
}

export function bn(value: bigint): anchor.BN {
  return new anchor.BN(value.toString());
}
