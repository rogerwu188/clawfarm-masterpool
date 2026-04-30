# Faucet Identity Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split faucet claim identities so the recipient wallet public key does not sign, the fee payer signs and pays SOL/rent, and the faucet source authority remains a separate token-source signer role.

**Architecture:** Keep the on-chain `claim_faucet` account names mostly stable to minimize IDL/client churn: `user` becomes a non-signer recipient account, `payer` remains the fee-payer signer, and `pool_authority` is documented and reported as the faucet source/token authority PDA. The TypeScript claim CLI exposes the clearer model as `--user-public-key` plus `--fee-payer-keypair`, creates recipient ATAs with the fee payer, and signs only with the fee payer.

**Tech Stack:** Anchor 0.32.1, Solana web3.js, SPL Token ATA helpers, TypeScript `tsx` scripts, `ts-mocha`/Chai tests, Rust Anchor program tests through `yarn test`.

---

## File Structure

- Modify `programs/clawfarm-masterpool/src/instructions/faucet.rs`: change `ClaimFaucet.user` from `Signer<'info>` to an unchecked recipient account, keep `payer: Signer<'info>`, clarify `pool_authority` as source/token authority, and continue deriving per-recipient state from `user.key()`.
- Modify `scripts/phase1/faucet-claim.ts`: replace the conflated `--user-keypair` flow with `--user-public-key` plus `--fee-payer-keypair`, use the fee payer for ATA creation/rent and transaction signing, and report `recipient`, `userPublicKey`, `feePayer`, and `sourceAuthority` separately.
- Modify `tests/phase1-faucet-script.ts`: update parser/report unit tests to assert the new CLI contract and identity fields.
- Modify `tests/phase1-integration.ts`: add a failing integration assertion that a fee payer can claim for a recipient without the recipient signature, then update existing faucet claim calls so they do not rely on recipient signatures.
- Modify `docs/phase1-testnet-runbook.md`: document the three faucet identities and the new claim command.

## Identity Model

- `recipient` / `userPublicKey`: wallet public key that receives `CLAW` and `Test USDC`; it does not sign faucet claims.
- `feePayer`: transaction signer that pays devnet SOL fees and ATA rent; it may be a server keypair or the same wallet as the recipient.
- `sourceAuthority` / `tokenAuthority`: faucet source authority that controls the faucet token accounts; in this program it is the existing `pool_authority` PDA, which signs SPL Token CPI transfers through program seeds rather than an external keypair.

### Task 1: Add failing CLI parser and report tests

**Files:**
- Modify: `tests/phase1-faucet-script.ts`
- Test: `tests/phase1-faucet-script.ts`

- [ ] **Step 1: Replace the faucet-claim parser tests with tests for separated identities**

In `tests/phase1-faucet-script.ts`, replace the whole `describe("faucet-claim parser", () => { ... });` block with this block:

```ts
describe("faucet-claim parser", () => {
  const recipient = "11111111111111111111111111111111";

  it("requires deployment, recipient public key, and fee payer keypair", () => {
    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--claw-amount",
      "1",
    ])).to.throw("user public key is required");

    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--claw-amount",
      "1",
    ])).to.throw("fee payer keypair path is required");
  });

  it("parses recipient public key, fee payer keypair, and ui claim amounts", () => {
    const args = parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--fee-payer-keypair",
      "/tmp/server-fee-payer.json",
      "--claw-amount",
      "1.5",
      "--usdc-amount",
      "2.25",
    ]);

    expect(args.userPublicKey.toBase58()).to.equal(recipient);
    expect(args.feePayerKeypair).to.equal("/tmp/server-fee-payer.json");
    expect(args.clawAmountBaseUnits.toString()).to.equal("1500000");
    expect(args.usdcAmountBaseUnits.toString()).to.equal("2250000");
  });

  it("requires at least one positive claim amount", () => {
    expect(() => parseFaucetClaimArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--user-public-key",
      recipient,
      "--fee-payer-keypair",
      "/tmp/server-fee-payer.json",
    ])).to.throw("at least one claim amount is required");
  });

  it("reports recipient, fee payer, source authority, and wallet daily remaining quota", () => {
    const report = buildFaucetClaimReport({
      signature: "sig",
      userPublicKey: "recipientWallet",
      feePayer: "serverFeePayer",
      sourceAuthority: "poolAuthorityPda",
      userFaucetState: "state",
      userClawToken: "clawAta",
      userUsdcToken: "usdcAta",
      clawAmountBaseUnits: BigInt("10000000"),
      usdcAmountBaseUnits: BigInt("5000000"),
      faucetConfig: {
        maxClawPerWalletPerDay: { toString: () => "50000000" },
        maxUsdcPerWalletPerDay: { toString: () => "50000000" },
      },
      userState: {
        currentDayIndex: { toString: () => "20567" },
        clawClaimedToday: { toString: () => "10000000" },
        usdcClaimedToday: { toString: () => "5000000" },
      },
    });

    expect(report.recipient).to.equal("recipientWallet");
    expect(report.userPublicKey).to.equal("recipientWallet");
    expect(report.feePayer).to.equal("serverFeePayer");
    expect(report.sourceAuthority).to.equal("poolAuthorityPda");
    expect(report.walletDailyQuota.claw.remaining).to.equal("40000000");
    expect(report.walletDailyQuota.usdc.remaining).to.equal("45000000");
  });
});
```

- [ ] **Step 2: Run the focused script tests and verify they fail**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-faucet-script.ts --grep "faucet-claim parser"
```

Expected: FAIL with TypeScript or assertion errors because `FaucetClaimArgs` still has `userKeypair`, `parseFaucetClaimArgs` does not require `--user-public-key` / `--fee-payer-keypair`, and `buildFaucetClaimReport` does not report separate identity fields.

- [ ] **Step 3: Commit the failing CLI tests**

```bash
git add tests/phase1-faucet-script.ts
git commit -m "test: cover separated faucet claim identities in cli"
```

### Task 2: Implement the separated identity claim CLI

**Files:**
- Modify: `scripts/phase1/faucet-claim.ts`
- Test: `tests/phase1-faucet-script.ts`

- [ ] **Step 1: Replace the argument interface**

In `scripts/phase1/faucet-claim.ts`, replace the current `FaucetClaimArgs` interface with:

```ts
export interface FaucetClaimArgs {
  deployment: string;
  userPublicKey: PublicKey;
  feePayerKeypair: string;
  clawAmountBaseUnits: bigint;
  usdcAmountBaseUnits: bigint;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}
```

- [ ] **Step 2: Replace the usage text**

In `scripts/phase1/faucet-claim.ts`, replace `usage()` with:

```ts
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
```

- [ ] **Step 3: Replace `parseFaucetClaimArgs`**

In `scripts/phase1/faucet-claim.ts`, replace the whole `parseFaucetClaimArgs` function with:

```ts
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
```

- [ ] **Step 4: Replace the report builder input and identity fields**

In `scripts/phase1/faucet-claim.ts`, replace the `buildFaucetClaimReport` input type's `user: string;` line with:

```ts
  userPublicKey: string;
  feePayer: string;
  sourceAuthority: string;
```

Then replace the first identity fields in the returned object:

```ts
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
```

Keep the existing `walletDailyQuota` body after `usdcAmount` unchanged.

- [ ] **Step 5: Replace the keypair/wallet and ATA creation flow in `main`**

In `scripts/phase1/faucet-claim.ts`, replace the old user keypair loading and wallet setup:

```ts
  const user = await loadKeypair(args.userKeypair);
  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(user);
```

with:

```ts
  const feePayer = await loadKeypair(args.feePayerKeypair);
  const userPublicKey = args.userPublicKey;
  const connection = new anchor.web3.Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(feePayer);
```

Then replace every later use in `main` as follows:

```ts
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
```

And replace the claim accounts/signers block with:

```ts
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
```

Finally replace the identity fields in the `buildFaucetClaimReport` call with:

```ts
        userPublicKey: userPublicKey.toBase58(),
        feePayer: feePayer.publicKey.toBase58(),
        sourceAuthority: pdas.poolAuthority.toBase58(),
```

- [ ] **Step 6: Run the focused script tests and verify they pass**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-faucet-script.ts --grep "faucet-claim parser"
```

Expected: PASS for all tests in `faucet-claim parser`.

- [ ] **Step 7: Run all faucet script tests**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-faucet-script.ts
```

Expected: PASS for the configure, fund, claim, and status parser/report tests.

- [ ] **Step 8: Commit the CLI implementation**

```bash
git add scripts/phase1/faucet-claim.ts tests/phase1-faucet-script.ts
git commit -m "feat: split faucet claim recipient and fee payer cli"
```

### Task 3: Add a failing integration test for recipient-without-signature claims

**Files:**
- Modify: `tests/phase1-integration.ts`
- Test: `tests/phase1-integration.ts`

- [ ] **Step 1: Add the failing integration test**

In `tests/phase1-integration.ts`, insert this test immediately after the existing test named `initializes, funds, enables, and enforces the devnet faucet`:

```ts
  it("lets a fee payer claim faucet tokens for a recipient without recipient signature", async () => {
    await ensureFaucetBootstrapped({ enabled: true, usdcVaultBalance: FAUCET_PER_WALLET_PER_DAY });

    const recipient = Keypair.generate();
    const recipientClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        recipient.publicKey
      )
    ).address;
    const recipientUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        recipient.publicKey
      )
    ).address;
    const [recipientFaucetState] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), recipient.publicKey.toBuffer()],
      masterpool.programId
    );

    await masterpool.methods
      .claimFaucet({
        clawAmount: new BN(1 * CLAW_UNIT),
        usdcAmount: new BN(2 * USDC_UNIT),
      })
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        faucetGlobalState: faucetGlobalPda,
        faucetUserState: recipientFaucetState,
        faucetClawVault: faucetClawVaultPda,
        faucetUsdcVault: faucetUsdcVaultPda,
        userClawToken: recipientClawAta,
        userUsdcToken: recipientUsdcAta,
        clawMint,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        user: recipient.publicKey,
        payer: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    const clawAccount = await getAccount(provider.connection, recipientClawAta);
    const usdcAccount = await getAccount(provider.connection, recipientUsdcAta);
    const userState = await (masterpool.account as any).faucetUserState.fetch(
      recipientFaucetState
    );

    assert.equal(clawAccount.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(usdcAccount.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(clawAccount.amount.toString(), String(1 * CLAW_UNIT));
    assert.equal(usdcAccount.amount.toString(), String(2 * USDC_UNIT));
    assert.equal(userState.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(userState.clawClaimedToday.toNumber(), 1 * CLAW_UNIT);
    assert.equal(userState.usdcClaimedToday.toNumber(), 2 * USDC_UNIT);
  });
```

- [ ] **Step 2: Run the focused integration test and verify it fails**

Run:

```bash
yarn test -- --grep "recipient without recipient signature"
```

Expected: FAIL with an Anchor account/signature error because `ClaimFaucet.user` is still a signer in `programs/clawfarm-masterpool/src/instructions/faucet.rs`.

- [ ] **Step 3: Commit the failing integration test**

```bash
git add tests/phase1-integration.ts
git commit -m "test: cover faucet claim recipient without signer"
```

### Task 4: Make the on-chain recipient non-signing and keep source authority separate

**Files:**
- Modify: `programs/clawfarm-masterpool/src/instructions/faucet.rs`
- Modify: `tests/phase1-integration.ts`
- Test: `tests/phase1-integration.ts`

- [ ] **Step 1: Change the `ClaimFaucet` accounts struct**

In `programs/clawfarm-masterpool/src/instructions/faucet.rs`, replace the bottom of `ClaimFaucet` from `pool_authority` through `system_program` with:

```rust
    /// CHECK: faucet source/token authority PDA for faucet vault transfers
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: recipient wallet public key; does not sign faucet claims
    pub user: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
```

Leave the existing `init_if_needed` constraint as `payer = payer` and `seeds = [FAUCET_USER_SEED, user.key().as_ref()]` so the fee payer funds the per-recipient faucet state account and the state remains keyed by the recipient.

- [ ] **Step 2: Make recipient intent explicit in `claim_faucet`**

In `programs/clawfarm-masterpool/src/instructions/faucet.rs`, inside `claim_faucet`, add this line immediately after `let faucet_config = &ctx.accounts.faucet_config;`:

```rust
    let recipient = ctx.accounts.user.key();
```

Then replace the `initialize_or_reset_user_if_needed` call with:

```rust
    initialize_or_reset_user_if_needed(
        &mut ctx.accounts.faucet_user_state,
        recipient,
        day_index,
        now,
    )?;
```

Replace the two token-owner checks with:

```rust
    require_token_owner(&ctx.accounts.user_claw_token, &recipient)?;
    require_token_mint(&ctx.accounts.user_claw_token, &ctx.accounts.config.claw_mint)?;
    require_token_owner(&ctx.accounts.user_usdc_token, &recipient)?;
    require_token_mint(&ctx.accounts.user_usdc_token, &ctx.accounts.config.usdc_mint)?;
```

- [ ] **Step 3: Update existing faucet claim calls to stop relying on recipient signatures**

In `tests/phase1-integration.ts`, for every `.claimFaucet(...)` account block in the faucet tests, keep `user: <recipient public key>` but change `payer` to `wallet.publicKey` unless the test is intentionally exercising another fee payer. Remove `.signers([faucetUser])` and `.signers([limitedUser])` from faucet claim calls.

The common account shape for faucet claims should be:

```ts
        poolAuthority: poolAuthorityPda,
        user: faucetUser.publicKey,
        payer: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
```

For the limited user daily-limit test, the account shape should be:

```ts
          poolAuthority: poolAuthorityPda,
          user: limitedUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
```

After this change, the only remaining `.signers([faucetUser])` in `tests/phase1-integration.ts` should be the invalid admin test that calls `setFaucetEnabled(false)` with `adminAuthority: faucetUser.publicKey`.

- [ ] **Step 4: Run the focused integration test and verify it passes**

Run:

```bash
yarn test -- --grep "recipient without recipient signature"
```

Expected: PASS. The recipient keypair is generated but never signs; `wallet.publicKey` pays the transaction and any created account rent.

- [ ] **Step 5: Run the full faucet integration group**

Run:

```bash
yarn test -- --grep "faucet"
```

Expected: PASS for faucet initialization, disabled claims, successful claims, invalid amount, per-claim limits, per-wallet daily limits, and recipient-without-signature coverage.

- [ ] **Step 6: Commit the on-chain identity split**

```bash
git add programs/clawfarm-masterpool/src/instructions/faucet.rs tests/phase1-integration.ts
git commit -m "feat: allow faucet fee payer separate from recipient"
```

### Task 5: Update the runbook with the three identity roles

**Files:**
- Modify: `docs/phase1-testnet-runbook.md`
- Test: `docs/phase1-testnet-runbook.md`

- [ ] **Step 1: Add the claim identity documentation**

In `docs/phase1-testnet-runbook.md`, after the faucet status command block, insert:

````md
Claim faucet tokens with separate recipient and fee-payer identities:

```bash
yarn phase1:faucet:claim --deployment deployments/devnet-phase1.json --user-public-key <recipient-wallet-pubkey> --fee-payer-keypair <server-or-user-fee-payer.json> --claw-amount 10 --usdc-amount 10
```

Faucet claim identities are intentionally separate:

- `recipient` / `userPublicKey`: wallet public key that receives test tokens in its associated token accounts; it does not sign.
- `feePayer`: keypair that signs the transaction and pays devnet SOL fees plus any ATA rent; this can be a server wallet or the same wallet as the recipient.
- `sourceAuthority` / `tokenAuthority`: the faucet token-source authority; in this program it is the existing `pool_authority` PDA that controls `faucet_claw_vault` and `faucet_usdc_vault`.
````

- [ ] **Step 2: Confirm the old claim flag is gone from docs and scripts**

Run:

```bash
rg --glob '!docs/superpowers/**' -- "--user-keypair" docs scripts tests
```

Expected: no matches for current docs, scripts, or tests because historical superpowers plans are excluded.

- [ ] **Step 3: Commit the documentation update**

```bash
git add docs/phase1-testnet-runbook.md
git commit -m "docs: document separated faucet claim identities"
```

### Task 6: Final verification

**Files:**
- Verify: `programs/clawfarm-masterpool/src/instructions/faucet.rs`
- Verify: `scripts/phase1/faucet-claim.ts`
- Verify: `tests/phase1-faucet-script.ts`
- Verify: `tests/phase1-integration.ts`
- Verify: `docs/phase1-testnet-runbook.md`

- [ ] **Step 1: Run TypeScript script tests**

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-faucet-script.ts
```

Expected: PASS.

- [ ] **Step 2: Run full local Anchor/Phase 1 tests**

```bash
yarn test
```

Expected: PASS. This rebuilds the programs, starts a local validator, deploys both programs, and runs `tests/phase1-integration.ts`.

- [ ] **Step 3: Inspect the generated IDL account model**

Run:

```bash
node - <<'NODE'
const idl = require('./target/idl/clawfarm_masterpool.json');
const ix = idl.instructions.find((item) => item.name === 'claimFaucet');
console.log(ix.accounts.map((account) => ({ name: account.name, signer: account.signer === true })));
NODE
```

Expected output includes `user` with `signer: false`, `payer` with `signer: true`, and `poolAuthority` with `signer: false` because the program signs for the PDA through seeds during CPI.

- [ ] **Step 4: Verify current claim CLI help**

Run:

```bash
yarn phase1:faucet:claim --help
```

Expected output includes `--user-public-key` and `--fee-payer-keypair`, and does not include `--user-keypair`.

- [ ] **Step 5: Commit any final verification fixes**

If verification required small corrections, commit them with:

```bash
git add programs/clawfarm-masterpool/src/instructions/faucet.rs scripts/phase1/faucet-claim.ts tests/phase1-faucet-script.ts tests/phase1-integration.ts docs/phase1-testnet-runbook.md
git commit -m "fix: complete faucet identity separation verification"
```

If there were no changes after Task 5, skip this commit.

## Self-Review

- Spec coverage: The plan covers all three requested identities. `recipient` / `userPublicKey` is represented by non-signer `ClaimFaucet.user`, recipient-owned ATAs, and CLI `--user-public-key`. `feePayer` is represented by `ClaimFaucet.payer`, CLI `--fee-payer-keypair`, ATA creation payer, and transaction signer. `tokenAuthority` / `sourceAuthority` is represented by the existing `pool_authority` PDA, clearer comments, and CLI report field `sourceAuthority`.
- Placeholder scan: The plan contains concrete file paths, commands, and code snippets rather than deferred work markers or vague implementation instructions.
- Type consistency: The plan consistently uses `userPublicKey: PublicKey`, `feePayerKeypair: string`, `feePayer: string`, `sourceAuthority: string`, `recipient` report output, `user` Anchor account as the recipient, `payer` Anchor account as the fee payer, and `poolAuthority` Anchor account as the PDA source authority.
