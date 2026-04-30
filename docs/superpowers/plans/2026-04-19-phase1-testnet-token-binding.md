# Phase 1 Testnet Token Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic testnet bootstrap tooling, operator-only Test USDC mint tooling, and explicit contract/test coverage so each Phase 1 deployment is permanently bound to one `CLAW` mint and one `Test USDC` mint.

**Architecture:** Keep the current on-chain single-pair mint binding model in `GlobalConfig`, but harden the `CLAW` genesis path with explicit mint-authority checks. Add TypeScript deployment scripts that create the two mints, transfer `CLAW` authority to the pool PDA, initialize both programs, mint the fixed `CLAW` genesis inventory, and record the resulting addresses in a deployment artifact. Add a separate operator script for external Test USDC minting, integration tests for wrong-mint rejection, and a runbook that documents the bootstrap sequence.

**Tech Stack:** Anchor 0.32, Rust, TypeScript, `@coral-xyz/anchor`, `@solana/web3.js`, `@solana/spl-token`, `ts-mocha`, `tsx`

---

## File Map

- Create: `scripts/phase1/common.ts`
  - Shared CLI helpers for keypair loading, PDA derivation, amount parsing, and deployment record persistence.
- Create: `scripts/phase1/bootstrap-testnet.ts`
  - Bootstrap script that creates the fixed mint pair, transfers `CLAW` authorities, initializes both programs, executes genesis minting, and writes the deployment record.
- Create: `scripts/phase1/mint-test-usdc.ts`
  - Operator-only script that mints `Test USDC` outside the contracts using the stored deployment record.
- Create: `tests/phase1-script-helpers.ts`
  - Unit tests for shared script helpers.
- Create: `tests/phase1-bootstrap-script.ts`
  - Unit tests for bootstrap argument parsing and validation.
- Create: `tests/phase1-test-usdc-script.ts`
  - Unit tests for Test USDC operator script argument parsing and amount handling.
- Modify: `package.json`
  - Add `tsx` plus runnable `phase1:*` script commands.
- Modify: `programs/clawfarm-masterpool/src/error.rs`
  - Add explicit custom errors for incorrect `CLAW` mint or freeze authority before genesis minting.
- Modify: `programs/clawfarm-masterpool/src/instructions/config.rs`
  - Enforce that `claw_mint.mint_authority` and `claw_mint.freeze_authority` both equal `pool_authority` before `mint_genesis_supply`.
- Modify: `tests/phase1-integration.ts`
  - Add explicit genesis-authority failure coverage and fixed-mint rejection coverage for rogue settlement mints.
- Create: `docs/phase1-testnet-runbook.md`
  - Operational runbook for deploying programs, bootstrapping mints, verifying authorities, and minting Test USDC.
- Modify: `programs/clawfarm-masterpool/README.md`
  - Link to the runbook from the contract documentation.

## Task 1: Add Shared Phase 1 Script Helpers

**Files:**
- Create: `scripts/phase1/common.ts`
- Test: `tests/phase1-script-helpers.ts`

- [ ] **Step 1: Write the failing helper tests**

```ts
import { expect } from "chai";
import { PublicKey } from "@solana/web3.js";

import {
  deriveMasterpoolPdas,
  toBaseUnits,
} from "../scripts/phase1/common";

describe("phase1 script helpers", () => {
  it("derives the same pool authority PDA as the on-chain seed", () => {
    const programId = new PublicKey("AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux");
    const expectedPoolAuthority = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_authority")],
      programId
    )[0];

    expect(deriveMasterpoolPdas(programId).poolAuthority.toBase58()).to.equal(
      expectedPoolAuthority.toBase58()
    );
  });

  it("parses six-decimal ui amounts into base units", () => {
    expect(toBaseUnits("1", 6).toString()).to.equal("1000000");
    expect(toBaseUnits("12.345678", 6).toString()).to.equal("12345678");
  });

  it("rejects ui amounts with too many fractional digits", () => {
    expect(() => toBaseUnits("0.0000001", 6)).to.throw(
      "too many decimal places"
    );
  });
});
```

- [ ] **Step 2: Run the helper test to verify it fails**

Run: `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-script-helpers.ts`

Expected: FAIL with `Cannot find module '../scripts/phase1/common'` or missing export errors for `deriveMasterpoolPdas` / `toBaseUnits`.

- [ ] **Step 3: Implement the shared helper module**

```ts
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

  return {
    masterpoolConfig,
    poolAuthority,
    rewardVault,
    challengeBondVault,
    treasuryUsdcVault,
    providerStakeUsdcVault,
    providerPendingUsdcVault,
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
```

- [ ] **Step 4: Run the helper test to verify it passes**

Run: `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-script-helpers.ts`

Expected: PASS with `3 passing`.

- [ ] **Step 5: Commit**

```bash
git add tests/phase1-script-helpers.ts scripts/phase1/common.ts
git commit -m "test: add phase1 script helpers"
```

### Task 2: Add the Fixed-Mint Bootstrap Script

**Files:**
- Create: `scripts/phase1/bootstrap-testnet.ts`
- Create: `tests/phase1-bootstrap-script.ts`
- Modify: `package.json`
- Modify: `scripts/phase1/common.ts`

- [ ] **Step 1: Write the failing bootstrap parser tests**

```ts
import { expect } from "chai";

import { parseBootstrapArgs } from "../scripts/phase1/bootstrap-testnet";

describe("bootstrap-testnet parser", () => {
  it("requires separate admin and test usdc operator keypairs", () => {
    expect(() =>
      parseBootstrapArgs([
        "--cluster",
        "devnet",
        "--rpc-url",
        "https://api.devnet.solana.com",
        "--admin-keypair",
        "/tmp/admin.json",
        "--test-usdc-operator-keypair",
        "/tmp/admin.json",
        "--masterpool-program-id",
        "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux",
        "--attestation-program-id",
        "52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2",
        "--out",
        "deployments/devnet-phase1.json",
      ])
    ).to.throw("must differ");
  });

  it("defaults the output path when --out is omitted", () => {
    const args = parseBootstrapArgs([
      "--cluster",
      "devnet",
      "--rpc-url",
      "https://api.devnet.solana.com",
      "--admin-keypair",
      "/tmp/admin.json",
      "--test-usdc-operator-keypair",
      "/tmp/operator.json",
      "--masterpool-program-id",
      "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux",
      "--attestation-program-id",
      "52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2",
    ]);

    expect(args.out).to.equal("deployments/devnet-phase1.json");
  });
});
```

- [ ] **Step 2: Run the bootstrap parser test to verify it fails**

Run: `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-bootstrap-script.ts`

Expected: FAIL with `Cannot find module '../scripts/phase1/bootstrap-testnet'` or missing export errors for `parseBootstrapArgs`.

- [ ] **Step 3: Implement the bootstrap script and package command**

```ts
import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import {
  AuthorityType,
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

export function parseBootstrapArgs(argv: string[]): BootstrapArgs {
  const valueOf = (flag: string): string | undefined => {
    const index = argv.indexOf(flag);
    return index === -1 ? undefined : argv[index + 1];
  };

  const cluster = valueOf("--cluster");
  const rpcUrl = valueOf("--rpc-url");
  const adminKeypair = valueOf("--admin-keypair");
  const testUsdcOperatorKeypair = valueOf("--test-usdc-operator-keypair");
  const masterpoolProgramId = valueOf("--masterpool-program-id");
  const attestationProgramId = valueOf("--attestation-program-id");
  const out = valueOf("--out") ?? "deployments/devnet-phase1.json";

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
  const args = parseBootstrapArgs(process.argv.slice(2));
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

  const masterpool = new anchor.Program(
    masterpoolIdl,
    args.masterpoolProgramId,
    provider
  );
  const attestation = new anchor.Program(
    attestationIdl,
    args.attestationProgramId,
    provider
  );

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
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .rpc();

  await masterpool.methods
    .mintGenesisSupply()
    .accounts({
      config: pdas.masterpoolConfig,
      adminAuthority: admin.publicKey,
      rewardVault: pdas.rewardVault,
      clawMint,
      poolAuthority: pdas.poolAuthority,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    } as any)
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
    console.error(error);
    process.exit(1);
  });
}
```

```json
{
  "scripts": {
    "phase1:bootstrap:testnet": "tsx scripts/phase1/bootstrap-testnet.ts"
  },
  "devDependencies": {
    "tsx": "^4.19.3"
  }
}
```

- [ ] **Step 4: Run the bootstrap tests and the missing-args smoke check**

Run:
- `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-bootstrap-script.ts`
- `yarn phase1:bootstrap:testnet`

Expected:
- test command PASS with `2 passing`
- script exits non-zero with `missing required bootstrap arguments`, proving the entrypoint is wired without touching chain state

- [ ] **Step 5: Commit**

```bash
git add package.json scripts/phase1/common.ts scripts/phase1/bootstrap-testnet.ts tests/phase1-bootstrap-script.ts
git commit -m "feat: add phase1 bootstrap script"
```

### Task 3: Add the Operator-Only Test USDC Mint Script

**Files:**
- Create: `scripts/phase1/mint-test-usdc.ts`
- Create: `tests/phase1-test-usdc-script.ts`
- Modify: `package.json`

- [ ] **Step 1: Write the failing Test USDC script tests**

```ts
import { expect } from "chai";

import { parseMintTestUsdcArgs } from "../scripts/phase1/mint-test-usdc";

describe("mint-test-usdc parser", () => {
  it("parses ui amounts with six decimals", () => {
    const args = parseMintTestUsdcArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--operator-keypair",
      "/tmp/operator.json",
      "--recipient",
      "11111111111111111111111111111111",
      "--amount",
      "15.250000",
    ]);

    expect(args.amountBaseUnits.toString()).to.equal("15250000");
  });

  it("requires a deployment record path", () => {
    expect(() =>
      parseMintTestUsdcArgs([
        "--operator-keypair",
        "/tmp/operator.json",
        "--recipient",
        "11111111111111111111111111111111",
        "--amount",
        "1",
      ])
    ).to.throw("deployment record is required");
  });
});
```

- [ ] **Step 2: Run the Test USDC parser test to verify it fails**

Run: `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-test-usdc-script.ts`

Expected: FAIL with `Cannot find module '../scripts/phase1/mint-test-usdc'` or missing export errors for `parseMintTestUsdcArgs`.

- [ ] **Step 3: Implement the operator mint script**

```ts
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

export function parseMintTestUsdcArgs(argv: string[]): MintTestUsdcArgs {
  const valueOf = (flag: string): string | undefined => {
    const index = argv.indexOf(flag);
    return index === -1 ? undefined : argv[index + 1];
  };

  const deployment = valueOf("--deployment");
  const operatorKeypair = valueOf("--operator-keypair");
  const recipient = valueOf("--recipient");
  const amount = valueOf("--amount");

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
  const args = parseMintTestUsdcArgs(process.argv.slice(2));
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
    console.error(error);
    process.exit(1);
  });
}
```

```json
{
  "scripts": {
    "phase1:mint:test-usdc": "tsx scripts/phase1/mint-test-usdc.ts"
  }
}
```

- [ ] **Step 4: Run the Test USDC parser tests and the missing-args smoke check**

Run:
- `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-test-usdc-script.ts`
- `yarn phase1:mint:test-usdc`

Expected:
- test command PASS with `2 passing`
- script exits non-zero with `deployment record is required`, proving the entrypoint is wired without minting

- [ ] **Step 5: Commit**

```bash
git add package.json scripts/phase1/mint-test-usdc.ts tests/phase1-test-usdc-script.ts
git commit -m "feat: add test usdc operator script"
```

### Task 4: Harden `mint_genesis_supply` with Explicit Authority Checks

**Files:**
- Modify: `programs/clawfarm-masterpool/src/error.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/config.rs`
- Modify: `tests/phase1-integration.ts`

- [ ] **Step 1: Write the failing integration assertion for missing `CLAW` authority transfer**

Add this block in `tests/phase1-integration.ts` immediately after the successful `initializeMasterpool()` call and before any `setAuthority(...)` calls for `clawMint`:

```ts
await expectAnchorError(
  masterpool.methods
    .mintGenesisSupply()
    .accounts({
      config: masterpoolConfigPda,
      adminAuthority: wallet.publicKey,
      rewardVault: rewardVaultPda,
      clawMint,
      poolAuthority: poolAuthorityPda,
      tokenProgram: TOKEN_PROGRAM_ID,
    } as any)
    .rpc(),
  "InvalidClawMintAuthority"
);
```

- [ ] **Step 2: Run the integration test to verify it fails**

Run: `./scripts/test-phase1.sh`

Expected: FAIL because `mint_genesis_supply` currently bubbles up an SPL token failure instead of the explicit `InvalidClawMintAuthority` error string.

- [ ] **Step 3: Add explicit authority checks and move the authority transfer into the test flow**

In `programs/clawfarm-masterpool/src/error.rs`, add:

```rust
    #[msg("The CLAW mint authority must be the pool authority before genesis minting")]
    InvalidClawMintAuthority,
    #[msg("The CLAW freeze authority must be the pool authority before genesis minting")]
    InvalidClawFreezeAuthority,
```

In `programs/clawfarm-masterpool/src/instructions/config.rs`, add the import and checks at the top of `mint_genesis_supply`:

```rust
use anchor_spl::token::spl_token::solana_program::program_option::COption;
```

```rust
    require!(
        ctx.accounts.claw_mint.mint_authority
            == COption::Some(ctx.accounts.pool_authority.key()),
        ErrorCode::InvalidClawMintAuthority
    );
    require!(
        ctx.accounts.claw_mint.freeze_authority
            == COption::Some(ctx.accounts.pool_authority.key()),
        ErrorCode::InvalidClawFreezeAuthority
    );
```

In `tests/phase1-integration.ts`, remove the eager `setAuthority(...)` calls from `before(...)` and insert them after the new failure assertion:

```ts
await setAuthority(
  provider.connection,
  wallet.payer,
  clawMint,
  wallet.payer,
  AuthorityType.MintTokens,
  poolAuthorityPda
);
await setAuthority(
  provider.connection,
  wallet.payer,
  clawMint,
  wallet.payer,
  AuthorityType.FreezeAccount,
  poolAuthorityPda
);
```

- [ ] **Step 4: Run the full integration test to verify it passes**

Run: `./scripts/test-phase1.sh`

Expected: PASS, including the new `InvalidClawMintAuthority` assertion followed by a successful `mintGenesisSupply()` call after the authority transfer.

- [ ] **Step 5: Commit**

```bash
git add programs/clawfarm-masterpool/src/error.rs programs/clawfarm-masterpool/src/instructions/config.rs tests/phase1-integration.ts
git commit -m "fix: validate claw authority before genesis mint"
```

### Task 5: Add Fixed-Mint Rejection Coverage

**Files:**
- Modify: `tests/phase1-integration.ts`

- [ ] **Step 1: Write the failing wrong-mint assertions**

Add new rogue-mint setup in `before(...)`:

```ts
let rogueUsdcMint: PublicKey;
let alternateProviderRogueUsdcAta: PublicKey;

rogueUsdcMint = await createMint(
  provider.connection,
  wallet.payer,
  wallet.publicKey,
  null,
  6
);

alternateProviderRogueUsdcAta = (
  await getOrCreateAssociatedTokenAccount(
    provider.connection,
    wallet.payer,
    rogueUsdcMint,
    alternateProviderWallet.publicKey
  )
).address;

await mintTo(
  provider.connection,
  wallet.payer,
  rogueUsdcMint,
  alternateProviderRogueUsdcAta,
  wallet.publicKey,
  1_000 * USDC_UNIT
);
```

Then add the two assertions after both programs are initialized and before the happy-path receipt lifecycle:

```ts
await expectAnchorError(
  masterpool.methods
    .registerProvider()
    .accounts({
      config: masterpoolConfigPda,
      providerAccount: alternateProviderAccountPda,
      providerRewardAccount: alternateProviderRewardPda,
      providerWallet: alternateProviderWallet.publicKey,
      providerStakeUsdcVault: providerStakeVaultPda,
      providerUsdcToken: alternateProviderRogueUsdcAta,
      usdcMint: rogueUsdcMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .signers([alternateProviderWallet])
    .rpc(),
  "InvalidUsdcMint"
);
```

```ts
const rogueSubmit = makeSubmitArgs("mint-mismatch");
rogueSubmit.chargeMint = rogueUsdcMint;

await expectAnchorError(
  submitReceipt("mint-mismatch", {
    submitArgs: rogueSubmit,
  }),
  "ChargeMintMismatch"
);
```

- [ ] **Step 2: Run the integration test to verify it fails**

Run: `./scripts/test-phase1.sh`

Expected: FAIL because `submitReceipt(...)` does not yet accept an injected `submitArgs` override and the new rogue mint setup has not been threaded into the helper.

- [ ] **Step 3: Thread the rogue mint setup and receipt override support through the test helper**

Extend the `submitReceipt` helper signature in `tests/phase1-integration.ts`:

```ts
  async function submitReceipt(
    requestNonce: string,
    overrides?: {
      payerUser?: Keypair;
      payerUsdcToken?: PublicKey;
      userRewardAccount?: PublicKey;
      providerWallet?: PublicKey;
      providerAccount?: PublicKey;
      providerRewardAccount?: PublicKey;
      submitArgs?: ReturnType<typeof makeSubmitArgs>;
    }
  ) {
    const receiptPda = deriveReceiptPda(requestNonce);
    const settlementPda = deriveReceiptSettlementPda(receiptPda);
    const submit = overrides?.submitArgs ?? makeSubmitArgs(requestNonce);
```

Keep the rest of the helper unchanged except for using `submit` instead of a freshly created value.

- [ ] **Step 4: Run the full integration suite to verify it passes**

Run: `./scripts/test-phase1.sh`

Expected: PASS, including:
- `InvalidUsdcMint` when a provider tries to stake with the rogue settlement mint
- `ChargeMintMismatch` when a receipt payload names a mint different from `config.usdc_mint`

- [ ] **Step 5: Commit**

```bash
git add tests/phase1-integration.ts
git commit -m "test: cover phase1 fixed mint rejections"
```

### Task 6: Add the Testnet Runbook and Documentation Links

**Files:**
- Create: `docs/phase1-testnet-runbook.md`
- Modify: `programs/clawfarm-masterpool/README.md`

- [ ] **Step 1: Add the failing documentation link check**

Run: `rg -n "phase1-testnet-runbook|phase1:bootstrap:testnet|phase1:mint:test-usdc" docs/phase1-testnet-runbook.md programs/clawfarm-masterpool/README.md`

Expected: FAIL because the runbook does not exist yet and the README has no link to it.

- [ ] **Step 2: Write the runbook and README reference**

Create `docs/phase1-testnet-runbook.md` with at least these sections and command blocks:

````md
# Phase 1 Testnet Runbook

## Prerequisites

- Anchor 0.32.1
- Solana CLI 3.1.12
- a funded admin keypair
- a separate funded Test USDC operator keypair

## 1. Build and deploy both programs

```bash
anchor build
solana program deploy target/deploy/clawfarm_masterpool.so --program-id target/deploy/clawfarm_masterpool-keypair.json --upgrade-authority <admin-keypair.json> --url https://api.devnet.solana.com
solana program deploy target/deploy/clawfarm_attestation.so --program-id target/deploy/clawfarm_attestation-keypair.json --upgrade-authority <admin-keypair.json> --url https://api.devnet.solana.com
```

## 2. Bootstrap the fixed mint pair

```bash
yarn phase1:bootstrap:testnet \
  --cluster devnet \
  --rpc-url https://api.devnet.solana.com \
  --admin-keypair <admin-keypair.json> \
  --test-usdc-operator-keypair <test-usdc-operator-keypair.json> \
  --masterpool-program-id AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux \
  --attestation-program-id 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2 \
  --out deployments/devnet-phase1.json
```

## 3. Verify the deployment record

- confirm `clawMint` and `testUsdcMint`
- confirm `poolAuthority`
- confirm `rewardVault`
- confirm `testUsdcOperator`

## 4. Mint Test USDC to a user

```bash
yarn phase1:mint:test-usdc \
  --deployment deployments/devnet-phase1.json \
  --operator-keypair <test-usdc-operator-keypair.json> \
  --recipient <RECIPIENT_PUBKEY> \
  --amount 250
```
````

Add this bullet under the documentation list in `programs/clawfarm-masterpool/README.md`:

```md
- [../../docs/phase1-testnet-runbook.md](../../docs/phase1-testnet-runbook.md)
```

- [ ] **Step 3: Run the documentation link check to verify it passes**

Run: `rg -n "phase1-testnet-runbook|phase1:bootstrap:testnet|phase1:mint:test-usdc" docs/phase1-testnet-runbook.md programs/clawfarm-masterpool/README.md`

Expected: PASS with matches in both the runbook and the README.

- [ ] **Step 4: Review the rendered markdown for accuracy**

Run: `sed -n '1,220p' docs/phase1-testnet-runbook.md && printf '\n---README---\n' && sed -n '1,40p' programs/clawfarm-masterpool/README.md`

Expected: The runbook shows the exact bootstrap/mint commands and the README links to the runbook near the existing source-of-truth links.

- [ ] **Step 5: Commit**

```bash
git add docs/phase1-testnet-runbook.md programs/clawfarm-masterpool/README.md
git commit -m "docs: add phase1 testnet runbook"
```

## Self-Review

- **Spec coverage:** The plan covers the chosen fixed double-mint design, the one-time `CLAW` genesis path, external Test USDC minting, deployment record persistence, explicit genesis-authority validation, wrong-mint rejection coverage, and operational documentation.
- **Placeholder scan:** No `TODO`, `TBD`, "implement later", or unnamed file references remain.
- **Type consistency:** The plan uses the same file names, function names, and script commands throughout: `deriveMasterpoolPdas`, `parseBootstrapArgs`, `parseMintTestUsdcArgs`, `phase1:bootstrap:testnet`, and `phase1:mint:test-usdc`.
