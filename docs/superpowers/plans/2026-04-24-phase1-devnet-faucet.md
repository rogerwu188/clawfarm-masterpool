# Phase 1 Devnet Faucet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a vault-backed devnet faucet in `clawfarm-masterpool` so users can claim limited `CLAW` and `Test USDC` from pre-funded vaults.

**Architecture:** Add separate faucet state accounts and a focused `instructions/faucet.rs` module without changing `GlobalConfig` layout. The faucet uses the existing `pool_authority` PDA to transfer from dedicated SPL vaults, stores one reusable global daily counter, and stores one reusable per-wallet daily counter. Devnet funding and status are handled by TypeScript scripts that perform ordinary SPL Token operations.

**Tech Stack:** Anchor 0.32.1, Rust Solana programs, `anchor-spl` token CPI, TypeScript phase1 scripts with `@coral-xyz/anchor`, `@solana/spl-token`, Mocha/Chai tests.

---

## File Structure

- Modify `programs/clawfarm-masterpool/src/constants.rs`: add faucet seeds and default base-unit limits.
- Modify `programs/clawfarm-masterpool/src/state/accounts.rs`: add `FaucetConfig`, `FaucetGlobalState`, and `FaucetUserState` account structs.
- Modify `programs/clawfarm-masterpool/src/state/types.rs`: add `FaucetLimits` and `FaucetClaimArgs` Anchor-serializable parameter structs.
- Modify `programs/clawfarm-masterpool/src/state/mod.rs`: export account space constants for the new faucet accounts.
- Modify `programs/clawfarm-masterpool/src/error.rs`: add faucet-specific errors.
- Create `programs/clawfarm-masterpool/src/instructions/faucet.rs`: implement initialization, admin configuration, daily counter reset, and claim transfer logic.
- Modify `programs/clawfarm-masterpool/src/instructions/mod.rs`: export faucet instruction account types.
- Modify `programs/clawfarm-masterpool/src/lib.rs`: expose faucet instructions from the Anchor program.
- Modify `scripts/phase1/common.ts`: derive faucet PDAs and extend deployment record with optional faucet addresses.
- Create `scripts/phase1/faucet-configure.ts`: initialize, enable/disable, and update faucet limits.
- Create `scripts/phase1/faucet-fund.ts`: fund faucet vaults using SPL transfer or USDC operator mint.
- Create `scripts/phase1/faucet-status.ts`: report faucet config, vault balances, and current global usage.
- Modify `scripts/phase1/post-smoke-validation.ts`: add mainnet faucet safety validation helper.
- Modify `package.json`: add phase1 faucet script commands.
- Modify `tests/phase1-integration.ts`: add program-level faucet tests.
- Create `tests/phase1-faucet-script.ts`: add parser and pure-helper tests for new scripts.
- Modify `tests/phase1-script-helpers.ts`: cover faucet PDA derivation.
- Modify `docs/phase1-testnet-runbook.md`: document devnet faucet setup, funding, claiming, and mainnet preflight rule.

---

### Task 1: Add Faucet State Types, Constants, and Errors

**Files:**
- Modify: `programs/clawfarm-masterpool/src/constants.rs`
- Modify: `programs/clawfarm-masterpool/src/state/accounts.rs`
- Modify: `programs/clawfarm-masterpool/src/state/types.rs`
- Modify: `programs/clawfarm-masterpool/src/state/mod.rs`
- Modify: `programs/clawfarm-masterpool/src/error.rs`

- [ ] **Step 1: Add failing Rust unit tests for faucet limit validation**

Add this test module to `programs/clawfarm-masterpool/src/utils.rs`, below the existing tests. These tests reference `validate_faucet_limits`, so they fail until Task 1 Step 3 adds it.

```rust
#[cfg(test)]
mod faucet_tests {
    use anchor_lang::error::Error;

    use super::validate_faucet_limits;
    use crate::{error::ErrorCode, state::FaucetLimits};

    fn valid_limits() -> FaucetLimits {
        FaucetLimits {
            max_claw_per_claim: 10_000_000,
            max_usdc_per_claim: 10_000_000,
            max_claw_per_wallet_per_day: 50_000_000,
            max_usdc_per_wallet_per_day: 50_000_000,
            max_claw_global_per_day: 50_000_000_000,
            max_usdc_global_per_day: 50_000_000_000,
        }
    }

    #[test]
    fn accepts_valid_faucet_limits() {
        validate_faucet_limits(&valid_limits()).unwrap();
    }

    #[test]
    fn rejects_zero_faucet_limits() {
        let mut limits = valid_limits();
        limits.max_claw_per_claim = 0;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }

    #[test]
    fn rejects_per_claim_above_wallet_daily() {
        let mut limits = valid_limits();
        limits.max_usdc_per_claim = 60_000_000;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }

    #[test]
    fn rejects_wallet_daily_above_global_daily() {
        let mut limits = valid_limits();
        limits.max_claw_per_wallet_per_day = 60_000_000_000;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }
}
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```bash
cargo test -p clawfarm-masterpool faucet_tests --lib
```

Expected: compile fails with unresolved imports for `validate_faucet_limits` and `FaucetLimits`.

- [ ] **Step 3: Add faucet constants, structs, account spaces, and errors**

Append these constants to `programs/clawfarm-masterpool/src/constants.rs`:

```rust
pub const FAUCET_CONFIG_SEED: &[u8] = b"faucet_config";
pub const FAUCET_GLOBAL_SEED: &[u8] = b"faucet_global";
pub const FAUCET_USER_SEED: &[u8] = b"faucet_user";
pub const FAUCET_CLAW_VAULT_SEED: &[u8] = b"faucet_claw_vault";
pub const FAUCET_USDC_VAULT_SEED: &[u8] = b"faucet_usdc_vault";

pub const DEFAULT_FAUCET_MAX_CLAW_PER_CLAIM: u64 = 10 * RATE_SCALE;
pub const DEFAULT_FAUCET_MAX_USDC_PER_CLAIM: u64 = 10 * RATE_SCALE;
pub const DEFAULT_FAUCET_MAX_CLAW_PER_WALLET_PER_DAY: u64 = 50 * RATE_SCALE;
pub const DEFAULT_FAUCET_MAX_USDC_PER_WALLET_PER_DAY: u64 = 50 * RATE_SCALE;
pub const DEFAULT_FAUCET_MAX_CLAW_GLOBAL_PER_DAY: u64 = 50_000 * RATE_SCALE;
pub const DEFAULT_FAUCET_MAX_USDC_GLOBAL_PER_DAY: u64 = 50_000 * RATE_SCALE;
pub const SECONDS_PER_UTC_DAY: i64 = 86_400;
```

Append these account structs to `programs/clawfarm-masterpool/src/state/accounts.rs`:

```rust
#[account]
#[derive(InitSpace)]
pub struct FaucetConfig {
    pub admin_authority: Pubkey,
    pub enabled: bool,
    pub faucet_claw_vault: Pubkey,
    pub faucet_usdc_vault: Pubkey,
    pub max_claw_per_claim: u64,
    pub max_usdc_per_claim: u64,
    pub max_claw_per_wallet_per_day: u64,
    pub max_usdc_per_wallet_per_day: u64,
    pub max_claw_global_per_day: u64,
    pub max_usdc_global_per_day: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct FaucetGlobalState {
    pub current_day_index: i64,
    pub claw_claimed_today: u64,
    pub usdc_claimed_today: u64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct FaucetUserState {
    pub owner: Pubkey,
    pub current_day_index: i64,
    pub claw_claimed_today: u64,
    pub usdc_claimed_today: u64,
    pub created_at: i64,
    pub updated_at: i64,
}
```

Append these parameter types to `programs/clawfarm-masterpool/src/state/types.rs`:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct FaucetLimits {
    pub max_claw_per_claim: u64,
    pub max_usdc_per_claim: u64,
    pub max_claw_per_wallet_per_day: u64,
    pub max_usdc_per_wallet_per_day: u64,
    pub max_claw_global_per_day: u64,
    pub max_usdc_global_per_day: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct FaucetClaimArgs {
    pub claw_amount: u64,
    pub usdc_amount: u64,
}
```

Append these space constants to `programs/clawfarm-masterpool/src/state/mod.rs`:

```rust
pub const FAUCET_CONFIG_SPACE: usize = 8 + FaucetConfig::INIT_SPACE;
pub const FAUCET_GLOBAL_STATE_SPACE: usize = 8 + FaucetGlobalState::INIT_SPACE;
pub const FAUCET_USER_STATE_SPACE: usize = 8 + FaucetUserState::INIT_SPACE;
```

Append these variants to `programs/clawfarm-masterpool/src/error.rs` before the closing brace:

```rust
    #[msg("The faucet is disabled")]
    FaucetDisabled,
    #[msg("The faucet claim amount is invalid")]
    InvalidFaucetAmount,
    #[msg("The faucet limits are invalid")]
    InvalidFaucetLimits,
    #[msg("The faucet per-claim limit was exceeded")]
    FaucetClaimLimitExceeded,
    #[msg("The faucet wallet daily limit was exceeded")]
    FaucetWalletDailyLimitExceeded,
    #[msg("The faucet global daily limit was exceeded")]
    FaucetGlobalDailyLimitExceeded,
    #[msg("The faucet vault balance is insufficient")]
    FaucetVaultInsufficientBalance,
    #[msg("The faucet vault account is invalid")]
    InvalidFaucetVault,
    #[msg("The faucet user state account is invalid")]
    InvalidFaucetUserState,
```

- [ ] **Step 4: Add faucet limit validation helper**

Add imports in `programs/clawfarm-masterpool/src/utils.rs`:

```rust
use crate::state::{FaucetLimits, GlobalConfig, Phase1ConfigParams, RewardAccount, RewardAccountKind};
```

Replace the existing `state::{...}` import with the line above. Then add this helper before `compute_linear_releasable_amount`:

```rust
pub fn validate_faucet_limits(limits: &FaucetLimits) -> Result<()> {
    require!(
        limits.max_claw_per_claim > 0
            && limits.max_usdc_per_claim > 0
            && limits.max_claw_per_wallet_per_day > 0
            && limits.max_usdc_per_wallet_per_day > 0
            && limits.max_claw_global_per_day > 0
            && limits.max_usdc_global_per_day > 0,
        ErrorCode::InvalidFaucetLimits
    );
    require!(
        limits.max_claw_per_claim <= limits.max_claw_per_wallet_per_day
            && limits.max_usdc_per_claim <= limits.max_usdc_per_wallet_per_day,
        ErrorCode::InvalidFaucetLimits
    );
    require!(
        limits.max_claw_per_wallet_per_day <= limits.max_claw_global_per_day
            && limits.max_usdc_per_wallet_per_day <= limits.max_usdc_global_per_day,
        ErrorCode::InvalidFaucetLimits
    );
    Ok(())
}
```

- [ ] **Step 5: Run unit tests and commit**

Run:

```bash
cargo test -p clawfarm-masterpool faucet_tests --lib
```

Expected: all four `faucet_tests` pass.

Commit:

```bash
git add programs/clawfarm-masterpool/src/constants.rs programs/clawfarm-masterpool/src/state/accounts.rs programs/clawfarm-masterpool/src/state/types.rs programs/clawfarm-masterpool/src/state/mod.rs programs/clawfarm-masterpool/src/error.rs programs/clawfarm-masterpool/src/utils.rs
git commit -m "feat: add faucet state and limits"
```

---

### Task 2: Implement Faucet Program Instructions

**Files:**
- Create: `programs/clawfarm-masterpool/src/instructions/faucet.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/mod.rs`
- Modify: `programs/clawfarm-masterpool/src/lib.rs`

- [ ] **Step 1: Add failing instruction-level tests to the integration suite**

In `tests/phase1-integration.ts`, add these constants near the existing token unit constants:

```ts
const FAUCET_PER_CLAIM = 10 * CLAW_UNIT;
const FAUCET_PER_WALLET_PER_DAY = 50 * CLAW_UNIT;
const FAUCET_GLOBAL_PER_DAY = 50_000 * CLAW_UNIT;
```

Add these variables near the existing PDA declarations:

```ts
  let faucetConfigPda: PublicKey;
  let faucetGlobalPda: PublicKey;
  let faucetUserPda: PublicKey;
  let faucetClawVaultPda: PublicKey;
  let faucetUsdcVaultPda: PublicKey;
  let faucetUser = Keypair.generate();
  let faucetUserClawAta: PublicKey;
  let faucetUserUsdcAta: PublicKey;
```

Add these PDA derivations in the `before` block after `poolAuthorityPda` is derived:

```ts
    [faucetConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_config")],
      masterpool.programId
    );
    [faucetGlobalPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_global")],
      masterpool.programId
    );
    [faucetClawVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_claw_vault")],
      masterpool.programId
    );
    [faucetUsdcVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_usdc_vault")],
      masterpool.programId
    );
    [faucetUserPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), faucetUser.publicKey.toBuffer()],
      masterpool.programId
    );
```

Add this airdrop in the `before` block with the other airdrops:

```ts
    await airdrop(faucetUser.publicKey);
```

Add these associated token account creations after the mints are created:

```ts
    faucetUserClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        faucetUser.publicKey
      )
    ).address;
    faucetUserUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        faucetUser.publicKey
      )
    ).address;
```

Add this `it` block after the current core lifecycle test. It references methods that do not exist yet, so it should fail before implementation:

```ts
  it("initializes, funds, enables, and enforces the devnet faucet", async () => {
    await masterpool.methods
      .initializeFaucet()
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        faucetGlobalState: faucetGlobalPda,
        faucetClawVault: faucetClawVaultPda,
        faucetUsdcVault: faucetUsdcVaultPda,
        clawMint,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        adminAuthority: wallet.publicKey,
        payer: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      } as any)
      .rpc();

    const initialConfig = await (masterpool.account as any).faucetConfig.fetch(
      faucetConfigPda
    );
    assert.equal(initialConfig.enabled, false);
    assert.equal(initialConfig.maxClawPerClaim.toNumber(), FAUCET_PER_CLAIM);
    assert.equal(initialConfig.maxUsdcPerClaim.toNumber(), FAUCET_PER_CLAIM);

    await mintTo(
      provider.connection,
      wallet.payer,
      clawMint,
      faucetClawVaultPda,
      wallet.payer,
      BigInt(100 * CLAW_UNIT)
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      faucetUsdcVaultPda,
      wallet.payer,
      BigInt(100 * USDC_UNIT)
    );

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(1 * CLAW_UNIT),
          usdcAmount: new BN(1 * USDC_UNIT),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: faucetUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([faucetUser])
        .rpc(),
      "FaucetDisabled"
    );

    await masterpool.methods
      .setFaucetEnabled(true)
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        adminAuthority: wallet.publicKey,
      } as any)
      .rpc();

    await masterpool.methods
      .claimFaucet({
        clawAmount: new BN(10 * CLAW_UNIT),
        usdcAmount: new BN(5 * USDC_UNIT),
      })
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        faucetGlobalState: faucetGlobalPda,
        faucetUserState: faucetUserPda,
        faucetClawVault: faucetClawVaultPda,
        faucetUsdcVault: faucetUsdcVaultPda,
        userClawToken: faucetUserClawAta,
        userUsdcToken: faucetUserUsdcAta,
        clawMint,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        user: faucetUser.publicKey,
        payer: faucetUser.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([faucetUser])
      .rpc();

    const clawAccount = await getAccount(provider.connection, faucetUserClawAta);
    const usdcAccount = await getAccount(provider.connection, faucetUserUsdcAta);
    assert.equal(clawAccount.amount.toString(), String(10 * CLAW_UNIT));
    assert.equal(usdcAccount.amount.toString(), String(5 * USDC_UNIT));

    const userState = await (masterpool.account as any).faucetUserState.fetch(
      faucetUserPda
    );
    assert.equal(userState.owner.toBase58(), faucetUser.publicKey.toBase58());
    assert.equal(userState.clawClaimedToday.toNumber(), 10 * CLAW_UNIT);
    assert.equal(userState.usdcClaimedToday.toNumber(), 5 * USDC_UNIT);

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(11 * CLAW_UNIT),
          usdcAmount: new BN(0),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: faucetUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([faucetUser])
        .rpc(),
      "FaucetClaimLimitExceeded"
    );
  });
```

If `expectAnchorError` does not already exist in the file, add this helper near the other test helpers:

```ts
async function expectAnchorError(
  promise: Promise<unknown>,
  expectedErrorName: string
): Promise<void> {
  try {
    await promise;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    assert.include(message, expectedErrorName);
    return;
  }
  assert.fail(`expected Anchor error ${expectedErrorName}`);
}
```

- [ ] **Step 2: Run integration test and verify it fails on missing IDL methods**

Run:

```bash
yarn test -- --grep "devnet faucet"
```

Expected: fail before sending transactions because `initializeFaucet`, `setFaucetEnabled`, or `claimFaucet` is missing from the IDL/client.

- [ ] **Step 3: Create `instructions/faucet.rs` implementation**

Create `programs/clawfarm-masterpool/src/instructions/faucet.rs` with this structure:

```rust
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{
        CONFIG_SEED, DEFAULT_FAUCET_MAX_CLAW_GLOBAL_PER_DAY,
        DEFAULT_FAUCET_MAX_CLAW_PER_CLAIM, DEFAULT_FAUCET_MAX_CLAW_PER_WALLET_PER_DAY,
        DEFAULT_FAUCET_MAX_USDC_GLOBAL_PER_DAY, DEFAULT_FAUCET_MAX_USDC_PER_CLAIM,
        DEFAULT_FAUCET_MAX_USDC_PER_WALLET_PER_DAY, FAUCET_CLAW_VAULT_SEED,
        FAUCET_CONFIG_SEED, FAUCET_GLOBAL_SEED, FAUCET_USER_SEED, FAUCET_USDC_VAULT_SEED,
        POOL_AUTHORITY_SEED, SECONDS_PER_UTC_DAY,
    },
    error::ErrorCode,
    state::{
        FaucetClaimArgs, FaucetConfig, FaucetGlobalState, FaucetLimits, FaucetUserState,
        GlobalConfig, FAUCET_CONFIG_SPACE, FAUCET_GLOBAL_STATE_SPACE, FAUCET_USER_STATE_SPACE,
    },
    utils::{checked_add_u64, require_token_mint, require_token_owner, validate_faucet_limits},
};

pub fn initialize_faucet(ctx: Context<InitializeFaucet>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let limits = FaucetLimits {
        max_claw_per_claim: DEFAULT_FAUCET_MAX_CLAW_PER_CLAIM,
        max_usdc_per_claim: DEFAULT_FAUCET_MAX_USDC_PER_CLAIM,
        max_claw_per_wallet_per_day: DEFAULT_FAUCET_MAX_CLAW_PER_WALLET_PER_DAY,
        max_usdc_per_wallet_per_day: DEFAULT_FAUCET_MAX_USDC_PER_WALLET_PER_DAY,
        max_claw_global_per_day: DEFAULT_FAUCET_MAX_CLAW_GLOBAL_PER_DAY,
        max_usdc_global_per_day: DEFAULT_FAUCET_MAX_USDC_GLOBAL_PER_DAY,
    };
    validate_faucet_limits(&limits)?;

    let faucet_config = &mut ctx.accounts.faucet_config;
    faucet_config.admin_authority = ctx.accounts.admin_authority.key();
    faucet_config.enabled = false;
    faucet_config.faucet_claw_vault = ctx.accounts.faucet_claw_vault.key();
    faucet_config.faucet_usdc_vault = ctx.accounts.faucet_usdc_vault.key();
    apply_faucet_limits(faucet_config, &limits);
    faucet_config.created_at = now;
    faucet_config.updated_at = now;

    let global = &mut ctx.accounts.faucet_global_state;
    global.current_day_index = current_day_index(now)?;
    global.claw_claimed_today = 0;
    global.usdc_claimed_today = 0;
    global.updated_at = now;

    Ok(())
}

pub fn set_faucet_enabled(ctx: Context<SetFaucetEnabled>, enabled: bool) -> Result<()> {
    let faucet_config = &mut ctx.accounts.faucet_config;
    faucet_config.enabled = enabled;
    faucet_config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn update_faucet_limits(ctx: Context<UpdateFaucetLimits>, limits: FaucetLimits) -> Result<()> {
    validate_faucet_limits(&limits)?;
    let faucet_config = &mut ctx.accounts.faucet_config;
    apply_faucet_limits(faucet_config, &limits);
    faucet_config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn claim_faucet(ctx: Context<ClaimFaucet>, args: FaucetClaimArgs) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let day_index = current_day_index(now)?;
    let faucet_config = &ctx.accounts.faucet_config;

    require!(faucet_config.enabled, ErrorCode::FaucetDisabled);
    require!(
        args.claw_amount > 0 || args.usdc_amount > 0,
        ErrorCode::InvalidFaucetAmount
    );
    require!(
        args.claw_amount <= faucet_config.max_claw_per_claim
            && args.usdc_amount <= faucet_config.max_usdc_per_claim,
        ErrorCode::FaucetClaimLimitExceeded
    );

    reset_global_if_needed(&mut ctx.accounts.faucet_global_state, day_index, now);
    initialize_or_reset_user_if_needed(
        &mut ctx.accounts.faucet_user_state,
        ctx.accounts.user.key(),
        day_index,
        now,
    )?;

    let next_user_claw = checked_add_u64(
        ctx.accounts.faucet_user_state.claw_claimed_today,
        args.claw_amount,
    )?;
    let next_user_usdc = checked_add_u64(
        ctx.accounts.faucet_user_state.usdc_claimed_today,
        args.usdc_amount,
    )?;
    require!(
        next_user_claw <= faucet_config.max_claw_per_wallet_per_day
            && next_user_usdc <= faucet_config.max_usdc_per_wallet_per_day,
        ErrorCode::FaucetWalletDailyLimitExceeded
    );

    let next_global_claw = checked_add_u64(
        ctx.accounts.faucet_global_state.claw_claimed_today,
        args.claw_amount,
    )?;
    let next_global_usdc = checked_add_u64(
        ctx.accounts.faucet_global_state.usdc_claimed_today,
        args.usdc_amount,
    )?;
    require!(
        next_global_claw <= faucet_config.max_claw_global_per_day
            && next_global_usdc <= faucet_config.max_usdc_global_per_day,
        ErrorCode::FaucetGlobalDailyLimitExceeded
    );

    require_token_owner(&ctx.accounts.user_claw_token, &ctx.accounts.user.key())?;
    require_token_mint(&ctx.accounts.user_claw_token, &ctx.accounts.config.claw_mint)?;
    require_token_owner(&ctx.accounts.user_usdc_token, &ctx.accounts.user.key())?;
    require_token_mint(&ctx.accounts.user_usdc_token, &ctx.accounts.config.usdc_mint)?;
    require!(
        ctx.accounts.faucet_claw_vault.amount >= args.claw_amount
            && ctx.accounts.faucet_usdc_vault.amount >= args.usdc_amount,
        ErrorCode::FaucetVaultInsufficientBalance
    );

    ctx.accounts.faucet_user_state.claw_claimed_today = next_user_claw;
    ctx.accounts.faucet_user_state.usdc_claimed_today = next_user_usdc;
    ctx.accounts.faucet_user_state.updated_at = now;
    ctx.accounts.faucet_global_state.claw_claimed_today = next_global_claw;
    ctx.accounts.faucet_global_state.usdc_claimed_today = next_global_usdc;
    ctx.accounts.faucet_global_state.updated_at = now;

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    if args.claw_amount > 0 {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.faucet_claw_vault.to_account_info(),
                    mint: ctx.accounts.claw_mint.to_account_info(),
                    to: ctx.accounts.user_claw_token.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer,
            ),
            args.claw_amount,
            ctx.accounts.claw_mint.decimals,
        )?;
    }

    if args.usdc_amount > 0 {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.faucet_usdc_vault.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.user_usdc_token.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer,
            ),
            args.usdc_amount,
            ctx.accounts.usdc_mint.decimals,
        )?;
    }

    Ok(())
}

pub fn current_day_index(unix_timestamp: i64) -> Result<i64> {
    unix_timestamp
        .checked_div(SECONDS_PER_UTC_DAY)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

fn apply_faucet_limits(config: &mut FaucetConfig, limits: &FaucetLimits) {
    config.max_claw_per_claim = limits.max_claw_per_claim;
    config.max_usdc_per_claim = limits.max_usdc_per_claim;
    config.max_claw_per_wallet_per_day = limits.max_claw_per_wallet_per_day;
    config.max_usdc_per_wallet_per_day = limits.max_usdc_per_wallet_per_day;
    config.max_claw_global_per_day = limits.max_claw_global_per_day;
    config.max_usdc_global_per_day = limits.max_usdc_global_per_day;
}

fn reset_global_if_needed(global: &mut FaucetGlobalState, day_index: i64, now: i64) {
    if global.current_day_index != day_index {
        global.current_day_index = day_index;
        global.claw_claimed_today = 0;
        global.usdc_claimed_today = 0;
        global.updated_at = now;
    }
}

fn initialize_or_reset_user_if_needed(
    user_state: &mut FaucetUserState,
    owner: Pubkey,
    day_index: i64,
    now: i64,
) -> Result<()> {
    if user_state.owner == Pubkey::default() {
        user_state.owner = owner;
        user_state.current_day_index = day_index;
        user_state.claw_claimed_today = 0;
        user_state.usdc_claimed_today = 0;
        user_state.created_at = now;
        user_state.updated_at = now;
        return Ok(());
    }
    require!(user_state.owner == owner, ErrorCode::InvalidFaucetUserState);
    if user_state.current_day_index != day_index {
        user_state.current_day_index = day_index;
        user_state.claw_claimed_today = 0;
        user_state.usdc_claimed_today = 0;
        user_state.updated_at = now;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeFaucet<'info> {
    #[account(seeds = [CONFIG_SEED], bump, has_one = admin_authority @ ErrorCode::UnauthorizedAdmin)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(init, payer = payer, space = FAUCET_CONFIG_SPACE, seeds = [FAUCET_CONFIG_SEED], bump)]
    pub faucet_config: Box<Account<'info, FaucetConfig>>,
    #[account(init, payer = payer, space = FAUCET_GLOBAL_STATE_SPACE, seeds = [FAUCET_GLOBAL_SEED], bump)]
    pub faucet_global_state: Box<Account<'info, FaucetGlobalState>>,
    #[account(init, payer = payer, token::mint = claw_mint, token::authority = pool_authority, seeds = [FAUCET_CLAW_VAULT_SEED], bump)]
    pub faucet_claw_vault: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = payer, token::mint = usdc_mint, token::authority = pool_authority, seeds = [FAUCET_USDC_VAULT_SEED], bump)]
    pub faucet_usdc_vault: Box<Account<'info, TokenAccount>>,
    #[account(constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint)]
    pub claw_mint: Box<Account<'info, Mint>>,
    #[account(constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer for program-owned vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub admin_authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SetFaucetEnabled<'info> {
    #[account(seeds = [CONFIG_SEED], bump, has_one = admin_authority @ ErrorCode::UnauthorizedAdmin)]
    pub config: Account<'info, GlobalConfig>,
    #[account(mut, seeds = [FAUCET_CONFIG_SEED], bump, has_one = admin_authority @ ErrorCode::UnauthorizedAdmin)]
    pub faucet_config: Account<'info, FaucetConfig>,
    pub admin_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFaucetLimits<'info> {
    #[account(seeds = [CONFIG_SEED], bump, has_one = admin_authority @ ErrorCode::UnauthorizedAdmin)]
    pub config: Account<'info, GlobalConfig>,
    #[account(mut, seeds = [FAUCET_CONFIG_SEED], bump, has_one = admin_authority @ ErrorCode::UnauthorizedAdmin)]
    pub faucet_config: Account<'info, FaucetConfig>,
    pub admin_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimFaucet<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [FAUCET_CONFIG_SEED], bump, has_one = faucet_claw_vault @ ErrorCode::InvalidFaucetVault, has_one = faucet_usdc_vault @ ErrorCode::InvalidFaucetVault)]
    pub faucet_config: Box<Account<'info, FaucetConfig>>,
    #[account(mut, seeds = [FAUCET_GLOBAL_SEED], bump)]
    pub faucet_global_state: Box<Account<'info, FaucetGlobalState>>,
    #[account(init_if_needed, payer = payer, space = FAUCET_USER_STATE_SPACE, seeds = [FAUCET_USER_SEED, user.key().as_ref()], bump)]
    pub faucet_user_state: Box<Account<'info, FaucetUserState>>,
    #[account(mut, address = faucet_config.faucet_claw_vault @ ErrorCode::InvalidFaucetVault, constraint = faucet_claw_vault.mint == config.claw_mint @ ErrorCode::InvalidTokenMint)]
    pub faucet_claw_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = faucet_config.faucet_usdc_vault @ ErrorCode::InvalidFaucetVault, constraint = faucet_usdc_vault.mint == config.usdc_mint @ ErrorCode::InvalidTokenMint)]
    pub faucet_usdc_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_claw_token: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_usdc_token: Box<Account<'info, TokenAccount>>,
    #[account(constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint)]
    pub claw_mint: Box<Account<'info, Mint>>,
    #[account(constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer for faucet vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
```

- [ ] **Step 4: Wire instructions into the module and program entrypoint**

Update `programs/clawfarm-masterpool/src/instructions/mod.rs`:

```rust
pub mod challenge;
pub mod config;
pub mod faucet;
pub mod provider;
pub mod receipt;
pub mod reward;

pub use challenge::{RecordChallengeBond, ResolveChallengeEconomics};
pub use config::{InitializeMasterpool, MintGenesisSupply, SetPauseFlags, UpdateConfig};
pub use faucet::{ClaimFaucet, InitializeFaucet, SetFaucetEnabled, UpdateFaucetLimits};
pub use provider::{ExitProvider, RegisterProvider};
pub use receipt::{RecordMiningFromReceipt, RecordMiningFromReceiptArgs, SettleFinalizedReceipt};
pub use reward::{ClaimReleasedClaw, MaterializeRewardRelease};
```

Update imports and exports in `programs/clawfarm-masterpool/src/lib.rs`:

```rust
#[allow(unused_imports)]
use instructions::faucet::{
    __client_accounts_claim_faucet, __client_accounts_initialize_faucet,
    __client_accounts_set_faucet_enabled, __client_accounts_update_faucet_limits,
    __cpi_client_accounts_claim_faucet, __cpi_client_accounts_initialize_faucet,
    __cpi_client_accounts_set_faucet_enabled, __cpi_client_accounts_update_faucet_limits,
};
```

Extend the public `use instructions::{...};` list with:

```rust
    ClaimFaucet, InitializeFaucet, SetFaucetEnabled, UpdateFaucetLimits,
```

Add program methods inside `pub mod clawfarm_masterpool`:

```rust
    pub fn initialize_faucet(ctx: Context<InitializeFaucet>) -> Result<()> {
        instructions::faucet::initialize_faucet(ctx)
    }

    pub fn set_faucet_enabled(ctx: Context<SetFaucetEnabled>, enabled: bool) -> Result<()> {
        instructions::faucet::set_faucet_enabled(ctx, enabled)
    }

    pub fn update_faucet_limits(
        ctx: Context<UpdateFaucetLimits>,
        limits: FaucetLimits,
    ) -> Result<()> {
        instructions::faucet::update_faucet_limits(ctx, limits)
    }

    pub fn claim_faucet(ctx: Context<ClaimFaucet>, args: FaucetClaimArgs) -> Result<()> {
        instructions::faucet::claim_faucet(ctx, args)
    }
```

- [ ] **Step 5: Run build and targeted faucet test**

Run:

```bash
anchor build
yarn test -- --grep "devnet faucet"
```

Expected: build succeeds and the faucet integration test passes.

- [ ] **Step 6: Commit**

```bash
git add programs/clawfarm-masterpool/src/instructions/faucet.rs programs/clawfarm-masterpool/src/instructions/mod.rs programs/clawfarm-masterpool/src/lib.rs tests/phase1-integration.ts
git commit -m "feat: add vault backed faucet instructions"
```

---

### Task 3: Add Full Faucet Limit and Safety Tests

**Files:**
- Modify: `tests/phase1-integration.ts`
- Modify: `programs/clawfarm-masterpool/src/instructions/faucet.rs`

- [ ] **Step 1: Add pure Rust tests for day-index reset helpers**

Make `reset_global_if_needed` and `initialize_or_reset_user_if_needed` `pub(crate)` in `programs/clawfarm-masterpool/src/instructions/faucet.rs`. Add this test module to the same file:

```rust
#[cfg(test)]
mod tests {
    use anchor_lang::prelude::*;

    use super::{initialize_or_reset_user_if_needed, reset_global_if_needed};
    use crate::state::{FaucetGlobalState, FaucetUserState};

    #[test]
    fn resets_global_state_when_day_changes() {
        let mut global = FaucetGlobalState {
            current_day_index: 10,
            claw_claimed_today: 7,
            usdc_claimed_today: 8,
            updated_at: 1,
        };
        reset_global_if_needed(&mut global, 11, 99);
        assert_eq!(global.current_day_index, 11);
        assert_eq!(global.claw_claimed_today, 0);
        assert_eq!(global.usdc_claimed_today, 0);
        assert_eq!(global.updated_at, 99);
    }

    #[test]
    fn keeps_global_state_when_day_matches() {
        let mut global = FaucetGlobalState {
            current_day_index: 10,
            claw_claimed_today: 7,
            usdc_claimed_today: 8,
            updated_at: 1,
        };
        reset_global_if_needed(&mut global, 10, 99);
        assert_eq!(global.claw_claimed_today, 7);
        assert_eq!(global.usdc_claimed_today, 8);
        assert_eq!(global.updated_at, 1);
    }

    #[test]
    fn initializes_empty_user_state_once() {
        let owner = Pubkey::new_unique();
        let mut user = FaucetUserState {
            owner: Pubkey::default(),
            current_day_index: 0,
            claw_claimed_today: 0,
            usdc_claimed_today: 0,
            created_at: 0,
            updated_at: 0,
        };
        initialize_or_reset_user_if_needed(&mut user, owner, 20, 100).unwrap();
        assert_eq!(user.owner, owner);
        assert_eq!(user.current_day_index, 20);
        assert_eq!(user.created_at, 100);
    }

    #[test]
    fn resets_user_state_when_day_changes() {
        let owner = Pubkey::new_unique();
        let mut user = FaucetUserState {
            owner,
            current_day_index: 20,
            claw_claimed_today: 5,
            usdc_claimed_today: 6,
            created_at: 10,
            updated_at: 11,
        };
        initialize_or_reset_user_if_needed(&mut user, owner, 21, 100).unwrap();
        assert_eq!(user.current_day_index, 21);
        assert_eq!(user.claw_claimed_today, 0);
        assert_eq!(user.usdc_claimed_today, 0);
        assert_eq!(user.created_at, 10);
        assert_eq!(user.updated_at, 100);
    }
}
```

- [ ] **Step 2: Run pure Rust faucet tests**

Run:

```bash
cargo test -p clawfarm-masterpool faucet::tests --lib
```

Expected: all four day-reset tests pass.

- [ ] **Step 3: Add integration tests for admin controls and invalid claims**

Add one `it` block to `tests/phase1-integration.ts` after the first faucet test:

```ts
  it("rejects invalid faucet admin and claim operations", async () => {
    await expectAnchorError(
      masterpool.methods
        .setFaucetEnabled(false)
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          adminAuthority: faucetUser.publicKey,
        } as any)
        .signers([faucetUser])
        .rpc(),
      "UnauthorizedAdmin"
    );

    await expectAnchorError(
      masterpool.methods
        .updateFaucetLimits({
          maxClawPerClaim: new BN(0),
          maxUsdcPerClaim: new BN(10 * USDC_UNIT),
          maxClawPerWalletPerDay: new BN(50 * CLAW_UNIT),
          maxUsdcPerWalletPerDay: new BN(50 * USDC_UNIT),
          maxClawGlobalPerDay: new BN(FAUCET_GLOBAL_PER_DAY),
          maxUsdcGlobalPerDay: new BN(FAUCET_GLOBAL_PER_DAY),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          adminAuthority: wallet.publicKey,
        } as any)
        .rpc(),
      "InvalidFaucetLimits"
    );

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({ clawAmount: new BN(0), usdcAmount: new BN(0) })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: faucetUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([faucetUser])
        .rpc(),
      "InvalidFaucetAmount"
    );
  });
```

- [ ] **Step 4: Add integration test for wallet daily limit**

Add this `it` block:

```ts
  it("enforces the faucet per-wallet daily limit", async () => {
    const limitedUser = Keypair.generate();
    await airdrop(limitedUser.publicKey);
    const limitedClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        limitedUser.publicKey
      )
    ).address;
    const limitedUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        limitedUser.publicKey
      )
    ).address;
    const [limitedUserPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), limitedUser.publicKey.toBuffer()],
      masterpool.programId
    );

    for (let index = 0; index < 5; index += 1) {
      await masterpool.methods
        .claimFaucet({
          clawAmount: new BN(10 * CLAW_UNIT),
          usdcAmount: new BN(0),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: limitedUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: limitedClawAta,
          userUsdcToken: limitedUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: limitedUser.publicKey,
          payer: limitedUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([limitedUser])
        .rpc();
    }

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(1 * CLAW_UNIT),
          usdcAmount: new BN(0),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: limitedUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: limitedClawAta,
          userUsdcToken: limitedUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: limitedUser.publicKey,
          payer: limitedUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([limitedUser])
        .rpc(),
      "FaucetWalletDailyLimitExceeded"
    );
  });
```

- [ ] **Step 5: Run targeted integration tests and commit**

Run:

```bash
yarn test -- --grep "faucet"
```

Expected: all faucet integration tests pass.

Commit:

```bash
git add programs/clawfarm-masterpool/src/instructions/faucet.rs tests/phase1-integration.ts
git commit -m "test: cover faucet limits and admin controls"
```

---

### Task 4: Add Faucet PDA Helpers and Script Parsers

**Files:**
- Modify: `scripts/phase1/common.ts`
- Create: `scripts/phase1/faucet-configure.ts`
- Create: `scripts/phase1/faucet-fund.ts`
- Create: `scripts/phase1/faucet-status.ts`
- Modify: `tests/phase1-script-helpers.ts`
- Create: `tests/phase1-faucet-script.ts`

- [ ] **Step 1: Add failing script helper tests**

Append to `tests/phase1-script-helpers.ts`:

```ts
  it("derives faucet PDAs", () => {
    const programId = new PublicKey("AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux");
    const pdas = deriveMasterpoolPdas(programId);

    expect(pdas.faucetConfig.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_config")], programId)[0].toBase58()
    );
    expect(pdas.faucetGlobal.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_global")], programId)[0].toBase58()
    );
    expect(pdas.faucetClawVault.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_claw_vault")], programId)[0].toBase58()
    );
    expect(pdas.faucetUsdcVault.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_usdc_vault")], programId)[0].toBase58()
    );
  });
```

Create `tests/phase1-faucet-script.ts`:

```ts
import { expect } from "chai";

import {
  DEFAULT_FAUCET_LIMITS,
  parseFaucetConfigureArgs,
} from "../scripts/phase1/faucet-configure";
import { parseFaucetFundArgs } from "../scripts/phase1/faucet-fund";
import { parseFaucetStatusArgs } from "../scripts/phase1/faucet-status";

describe("faucet-configure parser", () => {
  it("requires deployment and admin keypair", () => {
    expect(() => parseFaucetConfigureArgs(["--deployment", "deployments/devnet-phase1.json"])).to.throw(
      "admin keypair path is required"
    );
  });

  it("uses six-decimal default faucet limits", () => {
    expect(DEFAULT_FAUCET_LIMITS.maxClawPerClaim.toString()).to.equal("10000000");
    expect(DEFAULT_FAUCET_LIMITS.maxUsdcPerClaim.toString()).to.equal("10000000");
    expect(DEFAULT_FAUCET_LIMITS.maxClawGlobalPerDay.toString()).to.equal("50000000000");
    expect(DEFAULT_FAUCET_LIMITS.maxUsdcGlobalPerDay.toString()).to.equal("50000000000");
  });

  it("parses enable mode", () => {
    const args = parseFaucetConfigureArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--admin-keypair",
      "/tmp/admin.json",
      "--enable",
    ]);
    expect(args.enable).to.equal(true);
    expect(args.disable).to.equal(false);
  });
});

describe("faucet-fund parser", () => {
  it("requires a supported token", () => {
    expect(() => parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--funding-keypair",
      "/tmp/fund.json",
      "--token",
      "btc",
      "--amount",
      "1",
    ])).to.throw("token must be claw or usdc");
  });

  it("parses ui amount into base units", () => {
    const args = parseFaucetFundArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--funding-keypair",
      "/tmp/fund.json",
      "--token",
      "usdc",
      "--amount",
      "12.5",
    ]);
    expect(args.amountBaseUnits.toString()).to.equal("12500000");
  });
});

describe("faucet-status parser", () => {
  it("requires deployment", () => {
    expect(() => parseFaucetStatusArgs([])).to.throw("deployment path is required");
  });
});
```

- [ ] **Step 2: Run script tests and verify expected failures**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-script-helpers.ts tests/phase1-faucet-script.ts
```

Expected: compile fails because faucet scripts and PDA helper fields do not exist.

- [ ] **Step 3: Extend common PDA derivation and deployment type**

Modify `scripts/phase1/common.ts`:

```ts
const FAUCET_CONFIG_SEED = Buffer.from("faucet_config");
const FAUCET_GLOBAL_SEED = Buffer.from("faucet_global");
const FAUCET_CLAW_VAULT_SEED = Buffer.from("faucet_claw_vault");
const FAUCET_USDC_VAULT_SEED = Buffer.from("faucet_usdc_vault");
```

Add optional fields to `DeploymentRecord`:

```ts
  faucetConfig?: string;
  faucetGlobal?: string;
  faucetClawVault?: string;
  faucetUsdcVault?: string;
```

Extend the return object in `deriveMasterpoolPdas`:

```ts
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
```

Return:

```ts
    faucetConfig,
    faucetGlobal,
    faucetClawVault,
    faucetUsdcVault,
```

- [ ] **Step 4: Create faucet configure script parser and executable**

Create `scripts/phase1/faucet-configure.ts` with parser exports and a `main()` that loads the IDL, derives PDAs, and calls `initializeFaucet`, `setFaucetEnabled`, or `updateFaucetLimits` depending on flags. Use these exported defaults:

```ts
import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { PublicKey } from "@solana/web3.js";

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
```

The `main()` implementation should use `new anchor.Program(idl as anchor.Idl, provider)`, `deriveMasterpoolPdas`, `new PublicKey(deployment.clawMint)`, and `new PublicKey(deployment.testUsdcMint)`. It should print JSON containing `signature`, `faucetConfig`, `faucetGlobal`, `faucetClawVault`, `faucetUsdcVault`, and `enabled` when applicable.

- [ ] **Step 5: Create faucet fund script parser and executable**

Create `scripts/phase1/faucet-fund.ts` with parser exports:

```ts
import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { getAssociatedTokenAddressSync, mintTo, transfer } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";

import { DeploymentRecord, deriveMasterpoolPdas, loadKeypair, toBaseUnits } from "./common";

export type FaucetFundToken = "claw" | "usdc";

export interface FaucetFundArgs {
  deployment: string;
  fundingKeypair: string;
  token: FaucetFundToken;
  amountBaseUnits: bigint;
  rpcUrl?: string;
  masterpoolProgramId?: PublicKey;
}

function valueOf(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  return index === -1 ? undefined : argv[index + 1];
}

export function parseFaucetFundArgs(argv: string[]): FaucetFundArgs {
  const deployment = valueOf(argv, "--deployment");
  const fundingKeypair = valueOf(argv, "--funding-keypair");
  const token = valueOf(argv, "--token");
  const amount = valueOf(argv, "--amount");
  const rpcUrl = valueOf(argv, "--rpc-url");
  const masterpoolProgramId = valueOf(argv, "--masterpool-program-id");

  if (!deployment) throw new Error("deployment path is required");
  if (!fundingKeypair) throw new Error("funding keypair path is required");
  if (token !== "claw" && token !== "usdc") throw new Error("token must be claw or usdc");
  if (!amount) throw new Error("amount is required");

  return {
    deployment,
    fundingKeypair,
    token,
    amountBaseUnits: toBaseUnits(amount, 6),
    rpcUrl,
    masterpoolProgramId: masterpoolProgramId ? new PublicKey(masterpoolProgramId) : undefined,
  };
}
```

The `main()` implementation should:

- load deployment and signer
- derive faucet PDAs
- when `token === "claw"`, transfer from signer ATA to `pdas.faucetClawVault`
- when `token === "usdc"` and signer pubkey equals `deployment.testUsdcOperator`, call `mintTo` into `pdas.faucetUsdcVault`
- otherwise transfer from signer ATA to `pdas.faucetUsdcVault`
- print JSON with `signature`, `token`, `amount`, and `destinationVault`

- [ ] **Step 6: Create faucet status script parser and executable**

Create `scripts/phase1/faucet-status.ts` with parser exports:

```ts
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
```

The `main()` implementation should fetch `faucetConfig`, `faucetGlobalState`, the two vault token accounts, compute `currentDayIndex = Math.floor(Date.now() / 1000 / 86400)`, and print JSON. If `faucetConfig` is absent, print `{ "initialized": false }` and exit successfully.

- [ ] **Step 7: Run script parser tests and commit**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-script-helpers.ts tests/phase1-faucet-script.ts
```

Expected: all faucet script parser and PDA tests pass.

Commit:

```bash
git add scripts/phase1/common.ts scripts/phase1/faucet-configure.ts scripts/phase1/faucet-fund.ts scripts/phase1/faucet-status.ts tests/phase1-script-helpers.ts tests/phase1-faucet-script.ts
git commit -m "feat: add faucet operations scripts"
```

---

### Task 5: Add Package Commands, Mainnet Preflight Check, and Runbook

**Files:**
- Modify: `package.json`
- Modify: `scripts/phase1/post-smoke-validation.ts`
- Modify: `tests/phase1-post-smoke-validation-script.ts`
- Modify: `docs/phase1-testnet-runbook.md`

- [ ] **Step 1: Add failing tests for mainnet faucet safety helper**

In `tests/phase1-post-smoke-validation-script.ts`, import the helper:

```ts
import { validateMainnetFaucetDisabled } from "../scripts/phase1/post-smoke-validation";
```

Add these tests:

```ts
describe("mainnet faucet preflight", () => {
  it("passes when faucet config is absent", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", null)).not.to.throw();
  });

  it("passes on devnet even when faucet is enabled", () => {
    expect(() => validateMainnetFaucetDisabled("devnet", { enabled: true })).not.to.throw();
  });

  it("passes on mainnet when faucet is disabled", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", { enabled: false })).not.to.throw();
  });

  it("fails on mainnet when faucet is enabled", () => {
    expect(() => validateMainnetFaucetDisabled("mainnet-beta", { enabled: true })).to.throw(
      "mainnet faucet must not be enabled"
    );
  });
});
```

- [ ] **Step 2: Run test and verify expected failure**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-post-smoke-validation-script.ts --grep "mainnet faucet"
```

Expected: compile fails because `validateMainnetFaucetDisabled` is not exported.

- [ ] **Step 3: Implement mainnet preflight helper**

Add to `scripts/phase1/post-smoke-validation.ts` near other exported pure helpers:

```ts
export function validateMainnetFaucetDisabled(
  cluster: string,
  faucetConfig: { enabled: boolean } | null
): void {
  if (cluster !== "mainnet-beta") return;
  if (!faucetConfig) return;
  if (faucetConfig.enabled) {
    throw new Error("mainnet faucet must not be enabled");
  }
}
```

In the post-smoke validation flow, after loading the masterpool program and PDAs, fetch faucet config with `fetchNullable` when available:

```ts
const faucetConfig = await (masterpool.account as any).faucetConfig.fetchNullable(
  pdas.faucetConfig
);
validateMainnetFaucetDisabled(record.cluster, faucetConfig);
```

If `fetchNullable` is unavailable in the generated client, use a try/catch around `fetch` and treat an account-not-found error as `null`.

- [ ] **Step 4: Add package scripts**

Modify `package.json` scripts:

```json
"phase1:faucet:configure": "tsx scripts/phase1/faucet-configure.ts",
"phase1:faucet:fund": "tsx scripts/phase1/faucet-fund.ts",
"phase1:faucet:status": "tsx scripts/phase1/faucet-status.ts"
```

Keep existing scripts unchanged.

- [ ] **Step 5: Update runbook**

Append this section to `docs/phase1-testnet-runbook.md`:

```markdown
## Devnet Faucet

The faucet is a devnet/testnet convenience feature in `clawfarm-masterpool`. It is disabled by default and must not be enabled on mainnet.

Initialize and enable it on devnet:

```bash
yarn phase1:faucet:configure --deployment deployments/devnet-phase1.json --admin-keypair <admin.json> --initialize
yarn phase1:faucet:configure --deployment deployments/devnet-phase1.json --admin-keypair <admin.json> --enable
```

Fund `CLAW` from a wallet that already holds `CLAW`:

```bash
yarn phase1:faucet:fund --deployment deployments/devnet-phase1.json --funding-keypair <funding.json> --token claw --amount 1000
```

Fund `Test USDC` with the recorded operator wallet:

```bash
yarn phase1:faucet:fund --deployment deployments/devnet-phase1.json --funding-keypair <test-usdc-operator.json> --token usdc --amount 1000
```

Check faucet state and balances:

```bash
yarn phase1:faucet:status --deployment deployments/devnet-phase1.json
```

Initial limits use base units on chain and six-decimal UI amounts in scripts:

- per claim: `10 CLAW` and `10 Test USDC`
- per wallet per UTC day: `50 CLAW` and `50 Test USDC`
- global per UTC day: `50,000 CLAW` and `50,000 Test USDC`

Mainnet preflight must pass only when the faucet config is absent or present with `enabled == false`.
```

- [ ] **Step 6: Run validation and commit**

Run:

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-post-smoke-validation-script.ts --grep "mainnet faucet"
npx ts-mocha -p ./tsconfig.json tests/phase1-faucet-script.ts
```

Expected: tests pass.

Commit:

```bash
git add package.json scripts/phase1/post-smoke-validation.ts tests/phase1-post-smoke-validation-script.ts docs/phase1-testnet-runbook.md
git commit -m "chore: add faucet safety tooling and docs"
```

---

### Task 6: Full Verification

**Files:**
- No new files expected. Fix any failures in the files touched by Tasks 1-5.

- [ ] **Step 1: Run Rust unit tests**

```bash
cargo test -p clawfarm-masterpool --lib
```

Expected: all Rust unit tests pass.

- [ ] **Step 2: Run TypeScript parser and helper tests**

```bash
npx ts-mocha -p ./tsconfig.json tests/phase1-*-script.ts tests/phase1-script-helpers.ts
```

Expected: all TypeScript script tests pass.

- [ ] **Step 3: Run full local Phase 1 integration test**

```bash
yarn test
```

Expected: `anchor build` succeeds, local validator starts, both programs deploy, and `tests/phase1-integration.ts` passes.

- [ ] **Step 4: Inspect generated IDL for faucet methods**

```bash
node - <<'NODE'
const idl = require('./target/idl/clawfarm_masterpool.json');
const names = idl.instructions.map((ix) => ix.name).sort();
console.log(names.filter((name) => name.toLowerCase().includes('faucet')));
NODE
```

Expected output includes:

```text
[ 'claimFaucet', 'initializeFaucet', 'setFaucetEnabled', 'updateFaucetLimits' ]
```

- [ ] **Step 5: Commit verification fixes if needed**

If Step 1-4 required fixes, commit them:

```bash
git add programs/clawfarm-masterpool/src scripts/phase1 tests docs package.json
git commit -m "fix: complete faucet verification"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

Spec coverage:

- Vault-backed faucet in `clawfarm-masterpool`: Tasks 1-3.
- Separate `FaucetConfig`, `FaucetGlobalState`, `FaucetUserState`: Task 1.
- One reusable user account rather than one account per user per day: Tasks 1-3.
- UTC-day counter reset: Tasks 2-3.
- Per-claim, per-wallet daily, and global daily limits: Tasks 1-3.
- Default disabled and admin enable/disable: Tasks 2-3.
- Admin-configurable limits: Tasks 2-3.
- Funding by script, not chain instruction: Task 4.
- Status script: Task 4.
- Mainnet disabled preflight: Task 5.
- Runbook and package commands: Task 5.
- Full verification: Task 6.

Placeholder scan:

- No `TBD`, `TODO`, `implement later`, or intentionally incomplete task remains.
- The only conditional guidance is for Anchor client `fetchNullable` availability and provides a concrete fallback.

Type consistency:

- Rust structs use snake_case fields; Anchor TypeScript calls use camelCase fields.
- PDA seed names match the approved design and `common.ts` derivations.
- Default limits match `10/10`, `50/50`, and `50000/50000` multiplied by six-decimal precision.
