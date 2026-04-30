# Phase 1 Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three remaining Phase 1 review gaps: bind settlement identities to attested receipts, make reward release follow receipt-scoped lock snapshots, and reject unsafe economic configs before they land on-chain.

**Architecture:** Extend the attestation payload and registry so every economically settled receipt is cryptographically tied to one `payer_user` and one `provider_wallet`. Move reward release from a blind aggregate admin mutation to a receipt-scoped vesting calculation driven by `ReceiptSettlement` snapshots. Add explicit economic-bound validation so config writes fail closed instead of storing values that only explode during later settlement.

**Tech Stack:** Anchor 0.32.1, Rust, SPL Token CPI, TypeScript `anchor test`, Mocha/Chai

---

## File Map

- Modify: `programs/clawfarm-attestation/src/state/types.rs`
  - Add signed receipt identity fields.
- Modify: `programs/clawfarm-attestation/src/state/accounts.rs`
  - Persist payer/provider bindings on `Receipt`; bind `ProviderSigner` to one wallet.
- Modify: `programs/clawfarm-attestation/src/instructions/admin.rs`
  - Accept and store `provider_wallet` when registering a signer.
- Modify: `programs/clawfarm-attestation/src/instructions/receipt.rs`
  - Verify signed receipt identities against passed accounts before CPI to masterpool.
- Modify: `programs/clawfarm-attestation/src/tests.rs`
  - Cover canonical CBOR / identity binding regression cases.
- Modify: `programs/clawfarm-masterpool/src/state/accounts.rs`
  - Add receipt-level lock snapshots and per-receipt released counters.
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
  - Snapshot lock metadata at receipt record/finalize time.
- Modify: `programs/clawfarm-masterpool/src/instructions/reward.rs`
  - Replace arbitrary aggregate release with receipt-scoped linear vesting materialization.
- Modify: `programs/clawfarm-masterpool/src/utils.rs`
  - Add parameter-bound validation helpers and vesting math helpers.
- Modify: `programs/clawfarm-masterpool/src/constants.rs`
  - Add explicit supported settlement bounds.
- Modify: `tests/phase1-integration.ts`
  - Add spoofing tests, vesting tests, overflow-config tests, and make the local test entrypoint deterministic.
- Modify: `package.json`
  - Run the deterministic test entrypoint instead of plain `anchor test`.
- Create: `scripts/test-phase1.sh`
  - Explicit local deploy + test wrapper so bootstrap authorization tests use the expected upgrade authority.
- Modify: `docs/phase1-core-economics.md`
  - Document receipt-bound identities, receipt-scoped vesting, and parameter bounds.
- Modify: `programs/clawfarm-masterpool/README.md`
  - Document the new release flow and bounds.
- Modify: `programs/clawfarm-attestation/README.md`
  - Document provider-wallet binding and signed payer/provider identities.

### Task 1: Bind payer and provider identities into the attested receipt

**Files:**
- Modify: `programs/clawfarm-attestation/src/state/types.rs`
- Modify: `programs/clawfarm-attestation/src/state/accounts.rs`
- Modify: `programs/clawfarm-attestation/src/instructions/admin.rs`
- Modify: `programs/clawfarm-attestation/src/instructions/receipt.rs`
- Modify: `programs/clawfarm-attestation/src/tests.rs`
- Modify: `tests/phase1-integration.ts`

- [x] **Step 1: Write the failing regression tests**

Add a canonical-CBOR regression that proves `payer_user` and `provider_wallet` are now part of the signed payload, plus an integration case that submits a valid signed receipt but passes a different payer/provider account and expects rejection.

```rust
#[test]
fn canonical_cbor_includes_bound_identity_fields() {
    let mut args = sample_submit_receipt_args();
    args.payer_user = Pubkey::new_unique();
    args.provider_wallet = Pubkey::new_unique();

    let encoded = build_phase1_canonical_cbor(&args).unwrap();

    assert!(contains_subslice(&encoded, b"payer_user"));
    assert!(contains_subslice(&encoded, b"provider_wallet"));
}
```

```ts
it("rejects receipt settlement with spoofed payer/provider accounts", async () => {
  const spoofedPayer = Keypair.generate();
  const spoofedProvider = Keypair.generate();
  await airdrop(spoofedPayer.publicKey);

  await expectAnchorError(
    submitReceiptWithAccounts("phase1_spoofed_receipt", {
      payerUser: spoofedPayer,
      providerWallet: spoofedProvider.publicKey,
    }),
    "ReceiptIdentityMismatch"
  );
});
```

- [x] **Step 2: Run the tests to verify the new cases fail**

Run:

```bash
cargo test -q canonical_cbor_includes_bound_identity_fields
yarn test
```

Expected:

```text
failures:
    canonical_cbor_includes_bound_identity_fields
...
expected error containing ReceiptIdentityMismatch
```

- [x] **Step 3: Add receipt-bound identity fields and enforce them**

Extend the receipt args, persisted receipt state, and provider signer registry so the signed message and the runtime accounts must agree.

```rust
pub struct SubmitReceiptArgs {
    pub version: u8,
    pub proof_mode: u8,
    pub proof_id: String,
    pub request_nonce: String,
    pub provider: String,
    pub provider_wallet: Pubkey,
    pub payer_user: Pubkey,
    pub attester_type: u8,
    pub model: String,
    pub usage_basis: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub charge_atomic: u64,
    pub charge_mint: Pubkey,
    pub provider_request_id: Option<String>,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
}

pub struct ProviderSigner {
    pub provider_wallet: Pubkey,
    pub attester_type_mask: u8,
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
}

pub struct Receipt {
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
    pub payer_user: Pubkey,
    pub provider_wallet: Pubkey,
    pub submitted_at: i64,
    pub challenge_deadline: i64,
    pub finalized_at: i64,
    pub status: u8,
    pub economics_settled: bool,
}
```

```rust
// admin.rs
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_code: String,
    signer: Pubkey,
    provider_wallet: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()> {
    let provider_signer = &mut ctx.accounts.provider_signer;
    provider_signer.provider_wallet = provider_wallet;
    provider_signer.attester_type_mask = attester_type_mask;
    provider_signer.status = SignerStatus::Active as u8;
    provider_signer.valid_from = valid_from;
    provider_signer.valid_until = valid_until;
    Ok(())
}
```

```rust
// receipt.rs
require!(
    args.payer_user == ctx.accounts.payer_user.key()
        && args.provider_wallet == ctx.accounts.provider_wallet.key(),
    ErrorCode::ReceiptIdentityMismatch
);
require!(
    provider_signer.provider_wallet == args.provider_wallet,
    ErrorCode::ProviderWalletMismatch
);

receipt.payer_user = args.payer_user;
receipt.provider_wallet = args.provider_wallet;
```

```rust
// canonical cbor
("provider_wallet", CanonicalValue::Text(args.provider_wallet.to_string())),
("payer_user", CanonicalValue::Text(args.payer_user.to_string())),
```

- [x] **Step 4: Re-run the attestation and integration tests**

Run:

```bash
cargo test -q
yarn test
```

Expected:

```text
test result: ok.
...
Phase 1 core economics
  ...
```

- [x] **Step 5: Commit**

```bash
git add programs/clawfarm-attestation/src/state/types.rs \
  programs/clawfarm-attestation/src/state/accounts.rs \
  programs/clawfarm-attestation/src/instructions/admin.rs \
  programs/clawfarm-attestation/src/instructions/receipt.rs \
  programs/clawfarm-attestation/src/tests.rs \
  tests/phase1-integration.ts
git commit -m "fix: bind receipt settlement identities"
```

### Task 2: Make reward release receipt-scoped and driven by lock snapshots

**Files:**
- Modify: `programs/clawfarm-masterpool/src/state/accounts.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/reward.rs`
- Modify: `programs/clawfarm-masterpool/src/utils.rs`
- Modify: `tests/phase1-integration.ts`

- [x] **Step 1: Write the failing vesting tests**

Add one unit test for linear releasable math and one integration test proving a receipt cannot release more than its time-based tranche.

```rust
#[test]
fn linear_vesting_releases_only_elapsed_fraction() {
    let releasable = compute_linear_releasable_amount(
        100,
        0,
        0,
        5 * 86_400,
        10,
    )
    .unwrap();

    assert_eq!(releasable, 50);
}
```

```ts
it("rejects release above the receipt-scoped vested amount", async () => {
  const receipt = await submitReceipt("phase1_vesting_receipt");
  await waitForReceiptFinalizable(receipt.receiptPda);
  await finalizeReceipt(receipt.receiptPda, receipt.settlementPda);

  await expectAnchorError(
    materializeUserRelease(receipt.settlementPda),
    "RewardReleaseExceedsVested"
  );
});
```

```ts
async function materializeUserRelease(settlementPda: PublicKey) {
  await masterpool.methods
    .materializeRewardRelease(0)
    .accounts({
      config: masterpoolConfigPda,
      adminAuthority: wallet.publicKey,
      rewardAccount: userRewardPda,
      receiptSettlement: settlementPda,
    } as any)
    .rpc();
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test -q linear_vesting_releases_only_elapsed_fraction
yarn test
```

Expected:

```text
failures:
    linear_vesting_releases_only_elapsed_fraction
...
expected error containing RewardReleaseExceedsVested
```

- [x] **Step 3: Add receipt-level lock snapshots and per-receipt release accounting**

Store the historical lock config on each receipt and materialize release from that receipt only.

```rust
pub struct ReceiptSettlement {
    pub attestation_receipt: Pubkey,
    pub payer_user: Pubkey,
    pub provider_wallet: Pubkey,
    pub usdc_total_paid: u64,
    pub usdc_to_provider: u64,
    pub usdc_to_treasury: u64,
    pub claw_to_user: u64,
    pub claw_to_provider_total: u64,
    pub claw_provider_debt_offset: u64,
    pub claw_to_provider_locked: u64,
    pub lock_days_snapshot: u16,
    pub reward_lock_started_at: i64,
    pub user_claw_released: u64,
    pub provider_claw_released: u64,
    pub status: u8,
    pub created_at: i64,
    pub updated_at: i64,
}
```

```rust
// receipt.rs
settlement.lock_days_snapshot = config.lock_days;
settlement.reward_lock_started_at = 0;
settlement.user_claw_released = 0;
settlement.provider_claw_released = 0;

// finalize path
let now = Clock::get()?.unix_timestamp;
settlement.reward_lock_started_at = now;
settlement.updated_at = now;
```

```rust
pub enum ReleaseTarget {
    User = 0,
    Provider = 1,
}

pub fn compute_linear_releasable_amount(
    total_locked: u64,
    released_so_far: u64,
    lock_start: i64,
    now: i64,
    lock_days: u16,
) -> Result<u64> {
    let lock_seconds = i64::from(lock_days) * 86_400;
    let elapsed = (now - lock_start).clamp(0, lock_seconds);
    let vested = ((u128::from(total_locked) * u128::try_from(elapsed).unwrap())
        / u128::try_from(lock_seconds).unwrap()) as u64;
    checked_sub_u64(vested, released_so_far)
}
```

```rust
pub fn materialize_reward_release(
    ctx: Context<MaterializeRewardRelease>,
    target: u8,
) -> Result<()> {
    let target = ReleaseTarget::try_from(target)?;
    let settlement = &mut ctx.accounts.receipt_settlement;
    require!(settlement.reward_lock_started_at > 0, ErrorCode::RewardLockNotStarted);

    let now = Clock::get()?.unix_timestamp;
    let (total_locked, released_so_far) = match target {
        ReleaseTarget::User => (settlement.claw_to_user, settlement.user_claw_released),
        ReleaseTarget::Provider => (
            settlement.claw_to_provider_locked,
            settlement.provider_claw_released,
        ),
    };
    let releasable = compute_linear_releasable_amount(
        total_locked,
        released_so_far,
        settlement.reward_lock_started_at,
        now,
        settlement.lock_days_snapshot,
    )?;
    require!(releasable > 0, ErrorCode::RewardReleaseExceedsVested);
    // then move aggregate reward_account locked -> released and update receipt counters
    Ok(())
}
```

- [x] **Step 4: Re-run vesting and full-suite tests**

Run:

```bash
cargo test -q
yarn test
```

Expected:

```text
test result: ok.
...
0 failing
```

- [x] **Step 5: Commit**

```bash
git add programs/clawfarm-masterpool/src/state/accounts.rs \
  programs/clawfarm-masterpool/src/instructions/receipt.rs \
  programs/clawfarm-masterpool/src/instructions/reward.rs \
  programs/clawfarm-masterpool/src/utils.rs \
  tests/phase1-integration.ts
git commit -m "fix: materialize release from receipt lock snapshots"
```

### Task 3: Fail closed on unsafe economic parameter ranges

**Files:**
- Modify: `programs/clawfarm-masterpool/src/constants.rs`
- Modify: `programs/clawfarm-masterpool/src/utils.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/config.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Modify: `tests/phase1-integration.ts`

- [x] **Step 1: Write failing tests for overflow-prone configs**

Add one config test that tries to initialize/update with an absurd exchange rate and one receipt test that tries to exceed the supported per-receipt settlement bound.

```ts
await expectAnchorError(
  masterpool.methods
    .initializeMasterpool({
      exchangeRateClawPerUsdcE6: new BN("18446744073709551615"),
      providerStakeUsdc: new BN(PROVIDER_STAKE_USDC),
      providerUsdcShareBps: 700,
      treasuryUsdcShareBps: 300,
      userClawShareBps: 300,
      providerClawShareBps: 700,
      lockDays: 180,
      providerSlashClawAmount: new BN(PROVIDER_SLASH_CLAW),
      challengerRewardBps: 700,
      burnBps: 300,
      challengeBondClawAmount: new BN(CHALLENGE_BOND_CLAW),
    })
    .accounts({
      config: masterpoolConfigPda,
      rewardVault: rewardVaultPda,
      challengeBondVault: challengeBondVaultPda,
      treasuryUsdcVault: treasuryUsdcVaultPda,
      providerStakeUsdcVault: providerStakeVaultPda,
      providerPendingUsdcVault: providerPendingVaultPda,
      clawMint,
      usdcMint,
      attestationProgram: attestation.programId,
      selfProgram: masterpool.programId,
      selfProgramData: masterpoolProgramData,
      poolAuthority: poolAuthorityPda,
      initializer: wallet.publicKey,
      admin: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .rpc(),
  "InvalidGovernanceParameters"
);
```

```ts
const oversizedReceipt = Keypair.generate().publicKey;
const oversizedSettlement = deriveReceiptSettlementPda(oversizedReceipt);

await expectAnchorError(
  masterpool.methods
    .recordMiningFromReceipt({
      totalUsdcPaid: new BN("1000000000000001"),
      chargeMint: usdcMint,
    })
    .accounts({
      config: masterpoolConfigPda,
      attestationConfig: attestationConfigPda,
      payerUser: payerUser.publicKey,
      payerUsdcToken: payerUsdcAta,
      providerWallet: providerWallet.publicKey,
      providerAccount: providerAccountPda,
      providerRewardAccount: providerRewardPda,
      userRewardAccount: userRewardPda,
      receiptSettlement: oversizedSettlement,
      attestationReceipt: oversizedReceipt,
      treasuryUsdcVault: treasuryUsdcVaultPda,
      providerPendingUsdcVault: providerPendingVaultPda,
      usdcMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    } as any)
    .rpc(),
  "ReceiptChargeTooLarge"
);
```

- [x] **Step 2: Run the suite to verify the new checks fail first**

Run:

```bash
yarn test
```

Expected:

```text
expected error containing InvalidGovernanceParameters
expected error containing ReceiptChargeTooLarge
```

- [x] **Step 3: Add explicit supported bounds and validate them at config-write time**

Introduce a supported max receipt charge and prove all downstream math fits that domain.

```rust
pub const MAX_RECEIPT_USDC_ATOMIC: u64 = 1_000_000_000 * RATE_SCALE;
```

```rust
pub fn validate_phase1_params(params: &Phase1ConfigParams) -> Result<()> {
    // existing positive + split checks
    validate_phase1_param_bounds(params)?;
    Ok(())
}

pub fn validate_phase1_param_bounds(params: &Phase1ConfigParams) -> Result<()> {
    let max_total_claw = calculate_claw_amount(
        MAX_RECEIPT_USDC_ATOMIC,
        params.exchange_rate_claw_per_usdc_e6,
    )
    .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;

    let _max_user_claw = calculate_bps_amount(max_total_claw, params.user_claw_share_bps)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _max_provider_claw = calculate_bps_amount(max_total_claw, params.provider_claw_share_bps)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _challenger_reward = calculate_bps_amount(
        params.provider_slash_claw_amount,
        params.challenger_reward_bps,
    )
    .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _provider_slash_i128 = i128::try_from(params.provider_slash_claw_amount)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _challenge_bond_i128 = i128::try_from(params.challenge_bond_claw_amount)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    Ok(())
}
```

```rust
// receipt.rs
require!(
    args.total_usdc_paid <= crate::constants::MAX_RECEIPT_USDC_ATOMIC,
    ErrorCode::ReceiptChargeTooLarge
);
```

- [x] **Step 4: Re-run the config and settlement tests**

Run:

```bash
cargo test -q
yarn test
```

Expected:

```text
test result: ok.
...
0 failing
```

- [x] **Step 5: Commit**

```bash
git add programs/clawfarm-masterpool/src/constants.rs \
  programs/clawfarm-masterpool/src/utils.rs \
  programs/clawfarm-masterpool/src/instructions/config.rs \
  programs/clawfarm-masterpool/src/instructions/receipt.rs \
  tests/phase1-integration.ts
git commit -m "fix: reject unsafe phase1 economic configs"
```

### Task 4: Make the end-to-end test path deterministic and update docs

**Files:**
- Create: `scripts/test-phase1.sh`
- Modify: `package.json`
- Modify: `tests/phase1-integration.ts`
- Modify: `docs/phase1-core-economics.md`
- Modify: `programs/clawfarm-masterpool/README.md`
- Modify: `programs/clawfarm-attestation/README.md`

- [x] **Step 1: Add a deterministic local deploy-and-test wrapper**

Create a wrapper that explicitly builds, deploys, and then runs the test suite without a second deploy. This keeps the bootstrap-authorization test on the same upgrade authority that signed the deployment.

```bash
#!/usr/bin/env bash
set -euo pipefail

anchor build
anchor deploy
anchor test --skip-build --skip-deploy
```

- [x] **Step 2: Point `yarn test` at the wrapper and document the new semantics**

Update the package script and READMEs so contributors use the same deterministic flow.

```json
{
  "scripts": {
    "test": "bash ./scripts/test-phase1.sh"
  }
}
```

```md
- Receipts now bind both `payer_user` and `provider_wallet` inside the signed attestation payload.
- `ProviderSigner` is a `(provider_code, signer, provider_wallet)` binding, not just a `(provider_code, signer)` binding.
- Reward release is materialized per receipt using `lock_days_snapshot` and `reward_lock_started_at`.
- Config writes reject values that exceed the supported settlement bounds before they hit on-chain state.
```

- [x] **Step 3: Re-run the full test path and verify the docs mention every new invariant**

Run:

```bash
yarn test
rg -n "payer_user|provider_wallet|lock_days_snapshot|MAX_RECEIPT_USDC_ATOMIC" \
  docs/phase1-core-economics.md \
  programs/clawfarm-masterpool/README.md \
  programs/clawfarm-attestation/README.md
```

Expected:

```text
0 failing
...
docs/phase1-core-economics.md:...
programs/clawfarm-masterpool/README.md:...
programs/clawfarm-attestation/README.md:...
```

- [x] **Step 4: Commit**

```bash
git add scripts/test-phase1.sh \
  package.json \
  tests/phase1-integration.ts \
  docs/phase1-core-economics.md \
  programs/clawfarm-masterpool/README.md \
  programs/clawfarm-attestation/README.md
git commit -m "test: make phase1 verification deterministic"
```

## Self-Review

- Spec coverage:
  - Receipt identity binding is covered by Task 1.
  - Lock/release semantics and historical lock snapshotting are covered by Task 2.
  - Bounded parameter validation is covered by Task 3.
  - Rollout-readiness and reproducible verification are covered by Task 4.
- Placeholder scan:
  - No `TODO`, `TBD`, or “similar to above” markers remain.
- Type consistency:
  - `SubmitReceiptArgs` carries `provider_wallet` and `payer_user`.
  - `ReceiptSettlement` carries `lock_days_snapshot`, `reward_lock_started_at`, and receipt-level release counters.
  - `materialize_reward_release` becomes receipt-scoped and no longer accepts an arbitrary amount.

Plan complete and saved to `docs/superpowers/plans/2026-04-19-phase1-gap-closure.md`. Two execution options:

**1. Subagent-Driven (recommended)** - dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - execute tasks in this session in order, with checkpoints after each task
