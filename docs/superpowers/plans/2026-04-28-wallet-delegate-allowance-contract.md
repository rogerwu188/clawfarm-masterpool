# Wallet Delegate Allowance Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Status: Completed and merged to `main`
Completed: 2026-04-28
Implementation commits: `18f2c44`, `d56da60`, `2cf958a`, `0301576`, merged by `d2718f7`
Verification: `yarn test` passed with 11 integration tests on the merged result.

**Goal:** Update the ClawFarm attestation/masterpool contract path so a browser wallet can be recorded as `payer_user` and pay USDC through a pre-approved SPL Token delegate without signing `submit_receipt`.

**Architecture:** Keep the compact receipt identity unchanged: `payer_user` remains the browser wallet and stays in the receipt hash. Move SOL rent responsibility to a new `fee_payer` signer and SPL Token transfer authority to a new `payment_delegate` signer. The attestation program forwards both accounts into the masterpool CPI, while masterpool validates token owner, token mint, delegate pubkey, and delegated allowance before transferring USDC.

**Tech Stack:** Anchor 0.32.1, Rust Solana programs, SPL Token `TokenAccount.delegate` / `delegated_amount`, TypeScript Anchor integration tests, local `solana-test-validator` via `yarn test`.

---

## Scope

This plan is contract-only for `<clawfarm-masterpool-repo>`.

In scope:
- `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- `programs/clawfarm-masterpool/src/error.rs`
- `programs/clawfarm-attestation/src/instructions/receipt.rs`
- `tests/phase1-integration.ts`
- generated `target/idl/*.json` and `target/types/*.ts` from `anchor build`

Out of scope:
- `<AIRouter-repo>`
- `<clawfarm-site-repo>`
- devnet deployment
- faucet contract behavior

## File Structure

- `programs/clawfarm-masterpool/src/instructions/receipt.rs` owns receipt economics. It will accept `payer_user` as a non-signer identity, add `fee_payer` for PDA rent, add `payment_delegate` for SPL Token transfers, and validate delegate allowance.
- `programs/clawfarm-masterpool/src/error.rs` owns explicit masterpool errors. It will add `InvalidPaymentDelegate` and `InsufficientDelegatedAllowance`.
- `programs/clawfarm-attestation/src/instructions/receipt.rs` owns receipt verification and CPI into masterpool. It will stop requiring `payer_user` as a signer and pass `fee_payer` / `payment_delegate` into masterpool.
- `tests/phase1-integration.ts` owns local end-to-end coverage. It will approve a delegate, submit receipts without `payer_user` signing the receipt transaction, assert rent is paid by `fee_payer`, and assert wrong/low delegate allowance failures.
- `target/idl/clawfarm_attestation.json`, `target/idl/clawfarm_masterpool.json`, `target/types/clawfarm_attestation.ts`, and `target/types/clawfarm_masterpool.ts` are generated outputs used by downstream AIRouter helpers.

Implementation notes:
- The ABI keeps `fee_payer` and `payment_delegate` as separate accounts.
- Local tests default `payment_delegate` to the `fee_payer` keypair, matching the devnet MVP and avoiding legacy transaction size limits with the existing Ed25519 verification instruction.
- `anchor build` generated IDLs with the new accounts, but `target/idl` and `target/types` are not tracked in this repository checkout.

---

### Task 1: Write Failing Integration Tests For Delegate Settlement

**Files:**
- Modify: `tests/phase1-integration.ts`

- [x] **Step 1: Add the SPL Token approve helper import**

Change the SPL Token import in `tests/phase1-integration.ts` from:

```ts
import {
  AuthorityType,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  setAuthority,
  TOKEN_PROGRAM_ID,
  transferChecked,
} from "@solana/spl-token";
```

to:

```ts
import {
  AuthorityType,
  approveChecked,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  setAuthority,
  TOKEN_PROGRAM_ID,
  transferChecked,
} from "@solana/spl-token";
```

- [x] **Step 2: Add Gateway role keypairs to the test fixture**

Near the existing payer keypairs:

```ts
  const payerUser = Keypair.generate();
  const alternatePayerUser = Keypair.generate();
```

replace that block with:

```ts
  const payerUser = Keypair.generate();
  const alternatePayerUser = Keypair.generate();
  const feePayer = Keypair.generate();
  const paymentDelegate = Keypair.generate();
  const wrongPaymentDelegate = Keypair.generate();
```

In the `before(async () => { ... })` airdrop block, after:

```ts
    await airdrop(payerUser.publicKey);
    await airdrop(alternatePayerUser.publicKey);
```

add:

```ts
    await airdrop(feePayer.publicKey);
    await airdrop(paymentDelegate.publicKey);
    await airdrop(wrongPaymentDelegate.publicKey);
```

- [x] **Step 3: Add a success test that does not sign with `payer_user`**

Add this test before `it("submits with long off-chain metadata because only hashes go on chain", async () => {`:

```ts
  it("settles browser wallet receipts with fee payer and payment delegate while payer_user does not sign", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance(RECEIPT_CHARGE_USDC, paymentDelegate.publicKey);
    const payerLamportsBefore = await provider.connection.getBalance(payerUser.publicKey);
    const feePayerLamportsBefore = await provider.connection.getBalance(feePayer.publicKey);
    const payerUsdcBefore = await getAccount(provider.connection, payerUsdcAta);

    const receipt = await submitReceipt("delegate-success", {
      skipDelegateApproval: true,
    });

    const payerLamportsAfter = await provider.connection.getBalance(payerUser.publicKey);
    const feePayerLamportsAfter = await provider.connection.getBalance(feePayer.publicKey);
    const payerUsdcAfter = await getAccount(provider.connection, payerUsdcAta);
    const receiptState = await attestation.account.receipt.fetch(receipt.receiptPda);
    const settlementState = await masterpool.account.receiptSettlement.fetch(receipt.settlementPda);

    assert.equal(receiptState.payerUser.toBase58(), payerUser.publicKey.toBase58());
    assert.equal(settlementState.payerUser.toBase58(), payerUser.publicKey.toBase58());
    assert.equal(payerLamportsAfter, payerLamportsBefore, "payer_user must not fund receipt rent or transaction fees");
    assert.isBelow(feePayerLamportsAfter, feePayerLamportsBefore, "fee_payer should fund receipt settlement rent and transaction fees");
    assert.equal(
      payerUsdcAfter.amount.toString(),
      (payerUsdcBefore.amount - BigInt(RECEIPT_CHARGE_USDC)).toString()
    );
    assert.equal(payerUsdcAfter.delegatedAmount.toString(), "0");
  });
```

- [x] **Step 4: Add negative tests for wrong delegate and insufficient allowance**

Add these tests immediately after the success test from Step 3:

```ts
  it("rejects receipt settlement when the supplied payment delegate is not approved", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance(RECEIPT_CHARGE_USDC, paymentDelegate.publicKey);

    await expectAnchorError(
      submitReceipt("delegate-wrong", {
        paymentDelegate: wrongPaymentDelegate,
        skipDelegateApproval: true,
      }),
      "InvalidPaymentDelegate"
    );
  });

  it("rejects receipt settlement when delegated allowance is below the receipt charge", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance(RECEIPT_CHARGE_USDC - 1, paymentDelegate.publicKey);

    await expectAnchorError(
      submitReceipt("delegate-low-allowance", {
        skipDelegateApproval: true,
      }),
      "InsufficientDelegatedAllowance"
    );
  });
```

- [x] **Step 5: Add a helper that performs the browser-wallet approval transaction**

Add this helper near the other test helpers, before `async function submitReceipt(`:

```ts
  async function approvePayerAllowance(amount: number, delegate: PublicKey) {
    await approveChecked(
      provider.connection,
      wallet.payer,
      payerUsdcAta,
      usdcMint,
      delegate,
      payerUser,
      BigInt(amount),
      6
    );
  }
```

- [x] **Step 6: Update the `submitReceipt` helper signature to model Gateway roles**

In the `submitReceipt` helper override type, replace:

```ts
      signingKeypair?: Keypair;
      submitArgs?: ReturnType<typeof makeSubmitArgs>;
```

with:

```ts
      signingKeypair?: Keypair;
      feePayer?: Keypair;
      paymentDelegate?: Keypair;
      delegateAmount?: number;
      skipDelegateApproval?: boolean;
      submitArgs?: ReturnType<typeof makeSubmitArgs>;
```

At the beginning of the helper body, after `const settlementPda = deriveReceiptSettlementPda(receiptPda);`, add:

```ts
    const feePayerKeypair = overrides?.feePayer ?? feePayer;
    const paymentDelegateKeypair = overrides?.paymentDelegate ?? paymentDelegate;
    if (!overrides?.skipDelegateApproval) {
      await approvePayerAllowance(
        overrides?.delegateAmount ?? overrides?.chargeAtomic ?? RECEIPT_CHARGE_USDC,
        paymentDelegateKeypair.publicKey
      );
    }
```

- [x] **Step 7: Update `submitReceipt` accounts and signers for the new ABI**

Inside the `attestation.methods.submitReceipt(submit).accounts({ ... })` block, after:

```ts
        payerUser: overrides?.payerUser?.publicKey ?? payerUser.publicKey,
```

add:

```ts
        feePayer: feePayerKeypair.publicKey,
        paymentDelegate: paymentDelegateKeypair.publicKey,
```

Replace the transaction send block:

```ts
    const tx = new Transaction().add(ed25519Ix, submitIx);
    const signature = await provider.sendAndConfirm(tx, [overrides?.payerUser ?? payerUser]);
    return { receiptPda, settlementPda, signature, submitArgs: submit };
```

with:

```ts
    const tx = new Transaction().add(ed25519Ix, submitIx);
    tx.feePayer = feePayerKeypair.publicKey;
    const signature = await provider.sendAndConfirm(tx, [feePayerKeypair, paymentDelegateKeypair]);
    return { receiptPda, settlementPda, signature, submitArgs: submit };
```

This is the TDD failure point before contract changes: the current programs still require `payer_user` to sign and do not expose `fee_payer` / `payment_delegate` accounts.

- [x] **Step 8: Update direct masterpool test accounts for the new ABI**

In the unauthorized direct `masterpool.methods.recordMiningFromReceipt` test, after:

```ts
          payerUser: payerUser.publicKey,
```

add:

```ts
          feePayer: feePayer.publicKey,
          paymentDelegate: paymentDelegate.publicKey,
```

Replace:

```ts
        .signers([payerUser])
```

with:

```ts
        .signers([feePayer, paymentDelegate])
```

- [x] **Step 9: Run the focused integration test and confirm it fails for the old ABI**

Run:

```bash
yarn test -- --grep "fee payer and payment delegate|supplied payment delegate|delegated allowance"
```

Expected before implementation: FAIL. Acceptable failure signatures include `Signature verification failed`, an Anchor account resolution error for `feePayer` / `paymentDelegate`, or missing custom errors. The important point is that at least one new test fails against the current signer-based implementation.

- [x] **Step 10: Commit only the failing tests if working in a development branch**

Run:

```bash
git add tests/phase1-integration.ts
git commit -m "test: cover delegated receipt settlement roles"
```

If this repo has unrelated dirty files, stage only `tests/phase1-integration.ts`.

---

### Task 2: Update Masterpool Receipt Settlement ABI And Delegate Checks

**Files:**
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Modify: `programs/clawfarm-masterpool/src/error.rs`

- [x] **Step 1: Import `COption` for SPL Token delegate checks**

At the top of `programs/clawfarm-masterpool/src/instructions/receipt.rs`, change:

```rust
use anchor_lang::prelude::*;
```

to:

```rust
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
```

- [x] **Step 2: Add explicit masterpool errors**

In `programs/clawfarm-masterpool/src/error.rs`, add these variants after `InvalidTokenMint`:

```rust
    #[msg("The payment delegate is not approved for the payer token account")]
    InvalidPaymentDelegate,
    #[msg("The payment delegate allowance is below the receipt charge")]
    InsufficientDelegatedAllowance,
```

- [x] **Step 3: Add delegate validation before USDC transfers**

In `record_mining_from_receipt`, after:

```rust
    require_token_owner(&ctx.accounts.payer_usdc_token, &payer_user)?;
    require_token_mint(&ctx.accounts.payer_usdc_token, &config.usdc_mint)?;
```

add:

```rust
    require!(
        ctx.accounts.payer_usdc_token.delegate
            == COption::Some(ctx.accounts.payment_delegate.key()),
        ErrorCode::InvalidPaymentDelegate
    );
    require!(
        ctx.accounts.payer_usdc_token.delegated_amount >= args.total_usdc_paid,
        ErrorCode::InsufficientDelegatedAllowance
    );
```

- [x] **Step 4: Use `payment_delegate` as the SPL Token transfer authority**

In the treasury transfer, replace:

```rust
                authority: ctx.accounts.payer_user.to_account_info(),
```

with:

```rust
                authority: ctx.accounts.payment_delegate.to_account_info(),
```

In the provider-pending transfer, replace:

```rust
                authority: ctx.accounts.payer_user.to_account_info(),
```

with:

```rust
                authority: ctx.accounts.payment_delegate.to_account_info(),
```

- [x] **Step 5: Change `RecordMiningFromReceipt` accounts to separate identity, rent payer, and token authority**

In `RecordMiningFromReceipt<'info>`, replace this account block:

```rust
    #[account(mut)]
    pub payer_user: Signer<'info>,
    #[account(mut)]
    pub payer_usdc_token: Account<'info, TokenAccount>,
```

with:

```rust
    /// CHECK: business payer wallet, validated as the owner of payer_usdc_token
    #[account(mut)]
    pub payer_user: UncheckedAccount<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub payment_delegate: Signer<'info>,
    #[account(mut)]
    pub payer_usdc_token: Account<'info, TokenAccount>,
```

- [x] **Step 6: Charge receipt-created account rent to `fee_payer`**

In the `user_reward_account` account constraint, replace:

```rust
        payer = payer_user,
```

with:

```rust
        payer = fee_payer,
```

In the `receipt_settlement` account constraint, replace:

```rust
        payer = payer_user,
```

with:

```rust
        payer = fee_payer,
```

Keep the seeds unchanged:

```rust
        seeds = [USER_REWARD_SEED, payer_user.key().as_ref()],
```

and:

```rust
        seeds = [RECEIPT_SETTLEMENT_SEED, attestation_receipt.key().as_ref()],
```

- [x] **Step 7: Verify Rust formatting for the touched program files**

Run:

```bash
cargo fmt -- programs/clawfarm-masterpool/src/instructions/receipt.rs programs/clawfarm-masterpool/src/error.rs
```

Expected: command exits 0 and only formatting changes appear in the two listed files.

- [x] **Step 8: Run a Rust build and confirm attestation now needs CPI updates**

Run:

```bash
anchor build
```

Expected at this point: FAIL in `programs/clawfarm-attestation/src/instructions/receipt.rs` because the generated masterpool CPI account struct now requires `fee_payer` and `payment_delegate`.

- [x] **Step 9: Commit the masterpool ABI change if working in a development branch**

Run:

```bash
git add programs/clawfarm-masterpool/src/instructions/receipt.rs programs/clawfarm-masterpool/src/error.rs
git commit -m "feat: settle receipts through payment delegate"
```

If this repo has unrelated dirty files, stage only the two masterpool files.

---

### Task 3: Update Attestation Submit Receipt ABI And CPI Forwarding

**Files:**
- Modify: `programs/clawfarm-attestation/src/instructions/receipt.rs`

- [x] **Step 1: Forward the new masterpool accounts in the CPI**

Inside `clawfarm_masterpool::cpi::record_mining_from_receipt`, update the `MasterpoolRecordMiningFromReceipt { ... }` account struct.

Replace:

```rust
                payer_user: ctx.accounts.payer_user.to_account_info(),
                payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
```

with:

```rust
                payer_user: ctx.accounts.payer_user.to_account_info(),
                fee_payer: ctx.accounts.fee_payer.to_account_info(),
                payment_delegate: ctx.accounts.payment_delegate.to_account_info(),
                payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
```

- [x] **Step 2: Change `SubmitReceipt` accounts so `payer_user` is not a signer**

In `SubmitReceipt<'info>`, replace:

```rust
    #[account(mut)]
    pub payer_user: Signer<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub payer_usdc_token: UncheckedAccount<'info>,
```

with:

```rust
    /// CHECK: business payer wallet; validated by masterpool as payer_usdc_token owner
    #[account(mut)]
    pub payer_user: UncheckedAccount<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub payment_delegate: Signer<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub payer_usdc_token: UncheckedAccount<'info>,
```

Do not change receipt hash construction:

```rust
        payer_user: ctx.accounts.payer_user.key(),
```

Do not change receipt state storage:

```rust
    receipt.payer_user = ctx.accounts.payer_user.key();
```

Do not change receipt PDA rent payer:

```rust
        payer = authority,
```

- [x] **Step 3: Format the attestation file**

Run:

```bash
cargo fmt -- programs/clawfarm-attestation/src/instructions/receipt.rs
```

Expected: command exits 0 and only formatting changes appear in the attestation receipt file.

- [x] **Step 4: Build both programs and refresh IDLs/types**

Run:

```bash
anchor build
```

Expected: PASS. This updates:

```text
target/idl/clawfarm_attestation.json
target/idl/clawfarm_masterpool.json
target/types/clawfarm_attestation.ts
target/types/clawfarm_masterpool.ts
```

- [x] **Step 5: Confirm IDL account shape**

Run:

```bash
node - <<'NODE'
const fs = require('fs');
for (const file of ['target/idl/clawfarm_attestation.json', 'target/idl/clawfarm_masterpool.json']) {
  const idl = JSON.parse(fs.readFileSync(file, 'utf8'));
  const ix = idl.instructions.find((item) => item.name === 'submit_receipt' || item.name === 'submitReceipt' || item.name === 'record_mining_from_receipt' || item.name === 'recordMiningFromReceipt');
  const names = [];
  const walk = (items) => (items || []).forEach((item) => item.accounts ? walk(item.accounts) : names.push(item.name));
  walk(ix.accounts);
  console.log(file, names.filter((name) => ['payer_user', 'payerUser', 'fee_payer', 'feePayer', 'payment_delegate', 'paymentDelegate', 'payer_usdc_token', 'payerUsdcToken'].includes(name)).join(','));
}
NODE
```

Expected output contains each role for both instructions. For snake-case IDLs:

```text
target/idl/clawfarm_attestation.json payer_user,fee_payer,payment_delegate,payer_usdc_token
target/idl/clawfarm_masterpool.json payer_user,fee_payer,payment_delegate,payer_usdc_token
```

For camel-case IDLs, the same roles appear as `payerUser,feePayer,paymentDelegate,payerUsdcToken`.

- [x] **Step 6: Commit the attestation and generated IDL changes if working in a development branch**

Run:

```bash
git add programs/clawfarm-attestation/src/instructions/receipt.rs target/idl/clawfarm_attestation.json target/idl/clawfarm_masterpool.json target/types/clawfarm_attestation.ts target/types/clawfarm_masterpool.ts
git commit -m "feat: forward receipt fee payer and payment delegate"
```

If generated files are intentionally ignored in the active branch, commit only the Rust file and record the generated-file policy in the PR description.

---

### Task 4: Run Full Contract Verification And Fix Test Call Sites

**Files:**
- Modify as needed: `tests/phase1-integration.ts`
- Modify as needed: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Modify as needed: `programs/clawfarm-attestation/src/instructions/receipt.rs`

- [x] **Step 1: Run the full local contract suite**

Run:

```bash
yarn test
```

Expected after Tasks 1-3: PASS. The script performs `anchor build`, starts `solana-test-validator`, deploys both programs, and runs `tests/phase1-integration.ts`.

- [x] **Step 2: If an old test fails because it still expects `payer_user` signing, update that test to use Gateway role signers**

Search for old receipt submit signing patterns:

```bash
rg -n "signers\(\[payerUser\]\)|payerUser: payerUser.publicKey|recordMiningFromReceipt|submitReceipt\(" tests/phase1-integration.ts
```

For direct `recordMiningFromReceipt` calls, ensure accounts include:

```ts
          payerUser: payerUser.publicKey,
          feePayer: feePayer.publicKey,
          paymentDelegate: paymentDelegate.publicKey,
          payerUsdcToken: payerUsdcAta,
```

and signers are:

```ts
        .signers([feePayer, paymentDelegate])
```

For receipt submissions, route through the updated `submitReceipt` helper so the transaction signers are:

```ts
    const signature = await provider.sendAndConfirm(tx, [feePayerKeypair, paymentDelegateKeypair]);
```

- [x] **Step 3: If delegated allowance is consumed by multiple tests, approve per receipt**

Keep this logic in `submitReceipt` so every default receipt has fresh allowance:

```ts
    if (!overrides?.skipDelegateApproval) {
      await approvePayerAllowance(
        overrides?.delegateAmount ?? overrides?.chargeAtomic ?? RECEIPT_CHARGE_USDC,
        paymentDelegateKeypair.publicKey
      );
    }
```

Tests that intentionally set wrong or insufficient allowance must pass:

```ts
      skipDelegateApproval: true,
```

- [x] **Step 4: If challenge/finalize tests fail, do not add delegate roles to finalize/challenge APIs**

The delegate model only changes receipt submission. Challenge and finalize paths should keep their existing accounts. For challenge rejection refunds, the existing path still validates the payer token account by owner/mint and transfers from protocol-owned pending vaults back to `payer_usdc_token`; it does not need `payment_delegate`.

- [x] **Step 5: Re-run full verification after any fixes**

Run:

```bash
yarn test
```

Expected: PASS.

- [x] **Step 6: Commit test and verification fixes if working in a development branch**

Run:

```bash
git add tests/phase1-integration.ts programs/clawfarm-masterpool/src/instructions/receipt.rs programs/clawfarm-attestation/src/instructions/receipt.rs target/idl/clawfarm_attestation.json target/idl/clawfarm_masterpool.json target/types/clawfarm_attestation.ts target/types/clawfarm_masterpool.ts
git commit -m "test: verify browser wallet delegate settlement"
```

If some listed files have no changes, `git add` will ignore them.

---

### Task 5: Final Contract Review Checklist

**Files:**
- Review: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Review: `programs/clawfarm-attestation/src/instructions/receipt.rs`
- Review: `programs/clawfarm-masterpool/src/error.rs`
- Review: `tests/phase1-integration.ts`
- Review: `target/idl/clawfarm_attestation.json`
- Review: `target/idl/clawfarm_masterpool.json`

- [x] **Step 1: Verify `payer_user` is still in the receipt hash and stored receipt state**

Run:

```bash
rg -n "payer_user: ctx\.accounts\.payer_user\.key\(\)|receipt\.payer_user = ctx\.accounts\.payer_user\.key\(\)" programs/clawfarm-attestation/src/instructions/receipt.rs
```

Expected output contains both:

```text
payer_user: ctx.accounts.payer_user.key(),
receipt.payer_user = ctx.accounts.payer_user.key();
```

- [x] **Step 2: Verify `payer_user` is not a signer in submit/record accounts**

Run:

```bash
rg -n "pub payer_user: Signer|pub payer_user: UncheckedAccount|pub fee_payer: Signer|pub payment_delegate: Signer" programs/clawfarm-attestation/src/instructions/receipt.rs programs/clawfarm-masterpool/src/instructions/receipt.rs
```

Expected: no `pub payer_user: Signer` lines in receipt submit/record account structs; both files contain `pub payer_user: UncheckedAccount`, `pub fee_payer: Signer`, and `pub payment_delegate: Signer`.

- [x] **Step 3: Verify rent payer moved only in masterpool receipt-created accounts**

Run:

```bash
rg -n "payer = payer_user|payer = fee_payer|payer = authority" programs/clawfarm-masterpool/src/instructions/receipt.rs programs/clawfarm-attestation/src/instructions/receipt.rs
```

Expected:
- `programs/clawfarm-masterpool/src/instructions/receipt.rs` uses `payer = fee_payer` for `user_reward_account` and `receipt_settlement`.
- `programs/clawfarm-attestation/src/instructions/receipt.rs` keeps `payer = authority` for `receipt`.
- No receipt submit/record account uses `payer = payer_user`.

- [x] **Step 4: Verify delegate checks happen before transfers**

Run:

```bash
rg -n "InvalidPaymentDelegate|InsufficientDelegatedAllowance|payment_delegate\.to_account_info\(\)" programs/clawfarm-masterpool/src/instructions/receipt.rs programs/clawfarm-masterpool/src/error.rs
```

Expected output contains:

```text
InvalidPaymentDelegate
InsufficientDelegatedAllowance
payment_delegate.to_account_info()
payment_delegate.to_account_info()
```

- [x] **Step 5: Verify generated IDLs include new accounts**

Run the IDL check from Task 3 Step 5 again:

```bash
node - <<'NODE'
const fs = require('fs');
for (const file of ['target/idl/clawfarm_attestation.json', 'target/idl/clawfarm_masterpool.json']) {
  const idl = JSON.parse(fs.readFileSync(file, 'utf8'));
  const names = [];
  const walk = (items) => (items || []).forEach((item) => item.accounts ? walk(item.accounts) : names.push(item.name));
  for (const ix of idl.instructions) walk(ix.accounts);
  for (const required of ['fee_payer', 'payment_delegate']) {
    if (!names.includes(required) && !names.includes(required.replace(/_([a-z])/g, (_, c) => c.toUpperCase()))) {
      throw new Error(`${file} missing ${required}`);
    }
  }
  console.log(`${file}: ok`);
}
NODE
```

Expected:

```text
target/idl/clawfarm_attestation.json: ok
target/idl/clawfarm_masterpool.json: ok
```

- [x] **Step 6: Run final verification**

Run:

```bash
yarn test
```

Expected: PASS.

- [x] **Step 7: Capture final git state**

Run:

```bash
git status --short
```

Expected: only intentional files are modified. Do not revert unrelated pre-existing changes such as `tmp/phase1-smoketest.devnet.json` unless explicitly instructed by the user.

---

## Self-Review

- Spec coverage: The plan covers masterpool `payer_user` non-signer, `fee_payer` rent payer, `payment_delegate` SPL Token authority, delegate/allowance custom errors, attestation CPI forwarding, receipt hash identity preservation, generated IDLs, and local integration tests.
- Scope check: The plan is focused on contract changes only and excludes AIRouter/site/deployment changes.
- Placeholder scan: The plan contains concrete file paths, code snippets, commands, and expected outcomes; it does not rely on unspecified follow-up work.
- Type consistency: The account role names are consistently `payer_user` / `payerUser`, `fee_payer` / `feePayer`, `payment_delegate` / `paymentDelegate`, and `payer_usdc_token` / `payerUsdcToken` according to Rust and TypeScript/IDL naming conventions.
