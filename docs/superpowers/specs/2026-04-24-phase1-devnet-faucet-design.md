# Phase 1 Devnet Faucet Design

Status: Approved design for review
Date: 2026-04-24
Scope: Devnet/testnet faucet support for the current Phase 1 `clawfarm-masterpool` program
Related contracts and tooling:
- `programs/clawfarm-masterpool`
- `scripts/phase1`
- `deployments/devnet-phase1.json`

## 1. Current Devnet Token Authority State

The current devnet deployment uses these Phase 1 mints:

- `CLAW`: `GNWh9hfyEpbnNRzVdYBT7ZiB6VRJwXecSwTRohZByky8`
  - decimals: `6`
  - current mint authority: none
  - current freeze authority: none
  - total supply: `1_000_000_000 * 10^6` base units
  - supply was created by the one-time `mint_genesis_supply` flow
- `Test USDC`: `D3vhDe6mtdAgj2t8pu6XnaFXDPdiMDTALTSCZbizfm9P`
  - decimals: `6`
  - current mint authority: `<test-usdc-operator-pubkey>`
  - current freeze authority: none
  - this wallet is recorded as `testUsdcOperator` in `deployments/devnet-phase1.json`

Because `CLAW` mint authority has already been revoked, faucet delivery cannot mint new `CLAW`. The faucet must transfer existing `CLAW` from a funded vault. To avoid giving the program `Test USDC` mint authority, the faucet will also distribute `Test USDC` from a funded vault.

## 2. Goals

Add a devnet/testnet faucet that lets ordinary users claim `CLAW` and `Test USDC` directly through the `clawfarm-masterpool` program while enforcing all limits on-chain.

The faucet must enforce:

- per-claim `CLAW` and `Test USDC` maximums
- per-wallet UTC-day `CLAW` and `Test USDC` maximums
- global UTC-day `CLAW` and `Test USDC` maximums
- a program-controlled enabled flag so the faucet is disabled by default
- deployment validation that prevents a mainnet environment from shipping with the faucet enabled

Non-goals:

- minting additional `CLAW`
- moving `Test USDC` mint authority into the program
- supporting rolling 24-hour windows
- supporting production/mainnet faucet use
- building a generalized token faucet for arbitrary mints

## 3. Chosen Approach

Use a vault-backed faucet inside the existing `clawfarm-masterpool` program.

The program will own two faucet vaults through the existing `pool_authority` PDA:

- `faucet_claw_vault`, bound to `GlobalConfig.claw_mint`
- `faucet_usdc_vault`, bound to `GlobalConfig.usdc_mint`

Operators fund those vaults with normal SPL Token operations. Users then call `claim_faucet(claw_amount, usdc_amount)` to transfer tokens from the vaults into their token accounts.

This approach was chosen over two alternatives:

- Separate faucet program: cleaner isolation, but requires another program id, deployment flow, IDL, PDA set, and frontend integration for a small devnet-only feature.
- Off-chain faucet service: fastest to deploy, but limits would be server-enforced rather than chain-enforced.

The chosen approach keeps limits on-chain, reuses existing authorities and mint binding, and avoids restoring or transferring mint authority.

## 4. Time Window Rule

The faucet uses fixed UTC-day windows.

`day_index = floor(clock.unix_timestamp / 86400)`

When a claim occurs, the program compares the stored day index with the current day index:

- if the stored day is different, the relevant daily counters reset to zero and the stored day becomes the current day
- if the stored day is the same, the claim accumulates into the existing daily counters

This is not a rolling 24-hour window. It is a UTC calendar-day limit.

## 5. Accounts

### 5.1 `FaucetConfig`

Purpose: stores faucet configuration and limits.

PDA seed:

- `b"faucet_config"`

Fields:

- `admin_authority: Pubkey`
- `enabled: bool`
- `faucet_claw_vault: Pubkey`
- `faucet_usdc_vault: Pubkey`
- `max_claw_per_claim: u64`
- `max_usdc_per_claim: u64`
- `max_claw_per_wallet_per_day: u64`
- `max_usdc_per_wallet_per_day: u64`
- `max_claw_global_per_day: u64`
- `max_usdc_global_per_day: u64`
- `created_at: i64`
- `updated_at: i64`

`enabled` defaults to `false` during initialization.

The initial devnet limits are stored in base units because both mints use `6` decimals:

- max per claim:
  - `CLAW`: `10_000_000`
  - `Test USDC`: `10_000_000`
- max per wallet per UTC day:
  - `CLAW`: `50_000_000`
  - `Test USDC`: `50_000_000`
- max global per UTC day:
  - `CLAW`: `50_000_000_000`
  - `Test USDC`: `50_000_000_000`

### 5.2 `FaucetGlobalState`

Purpose: tracks global usage for the current UTC day.

PDA seed:

- `b"faucet_global"`

Fields:

- `current_day_index: i64`
- `claw_claimed_today: u64`
- `usdc_claimed_today: u64`
- `updated_at: i64`

This account is reused across days. It does not create one account per day.

### 5.3 `FaucetUserState`

Purpose: tracks one wallet's usage for the current UTC day.

PDA seed:

- `b"faucet_user"`
- `user_wallet.key().as_ref()`

Fields:

- `owner: Pubkey`
- `current_day_index: i64`
- `claw_claimed_today: u64`
- `usdc_claimed_today: u64`
- `created_at: i64`
- `updated_at: i64`

This account is initialized once per wallet with `init_if_needed` during the first claim and reused on later days. This avoids the `users * days` account growth of daily per-user PDAs while still making per-wallet limits enforceable on-chain.

### 5.4 Faucet Vaults

Two SPL Token accounts are created during faucet initialization:

- `faucet_claw_vault`
  - mint: `GlobalConfig.claw_mint`
  - authority: existing `pool_authority` PDA
- `faucet_usdc_vault`
  - mint: `GlobalConfig.usdc_mint`
  - authority: existing `pool_authority` PDA

These vaults are dedicated to faucet distribution. Claim instructions transfer from these vaults into user-owned token accounts.

## 6. Instructions

### 6.1 `initialize_faucet`

Caller: `GlobalConfig.admin_authority`

Creates:

- `FaucetConfig`
- `FaucetGlobalState`
- `faucet_claw_vault`
- `faucet_usdc_vault`

Rules:

- caller must match `GlobalConfig.admin_authority`
- `faucet_claw_vault.mint` must equal `GlobalConfig.claw_mint`
- `faucet_usdc_vault.mint` must equal `GlobalConfig.usdc_mint`
- both vault authorities must be the existing `pool_authority` PDA
- default `enabled` is `false`
- default limits use the base-unit values in section 5.1

### 6.2 `set_faucet_enabled(enabled)`

Caller: `GlobalConfig.admin_authority`

Updates `FaucetConfig.enabled`.

Expected use:

- devnet setup scripts call this with `true`
- mainnet deployment validation fails if this value is `true`

### 6.3 `update_faucet_limits(params)`

Caller: `GlobalConfig.admin_authority`

Updates all six limit values.

Validation:

- every limit must be greater than zero
- `max_claw_per_claim <= max_claw_per_wallet_per_day`
- `max_usdc_per_claim <= max_usdc_per_wallet_per_day`
- `max_claw_per_wallet_per_day <= max_claw_global_per_day`
- `max_usdc_per_wallet_per_day <= max_usdc_global_per_day`

A token should be disabled by setting `enabled = false` for the faucet as a whole, not by setting one limit to zero.

### 6.4 `claim_faucet(claw_amount, usdc_amount)`

Caller: ordinary user

The user may claim both tokens in one transaction. Either amount may be zero, but both amounts cannot be zero.

Rules:

- `FaucetConfig.enabled == true`
- `claw_amount > 0 || usdc_amount > 0`
- `claw_amount <= max_claw_per_claim`
- `usdc_amount <= max_usdc_per_claim`
- after any day reset, `user_state.claw_claimed_today + claw_amount <= max_claw_per_wallet_per_day`
- after any day reset, `user_state.usdc_claimed_today + usdc_amount <= max_usdc_per_wallet_per_day`
- after any day reset, `global_state.claw_claimed_today + claw_amount <= max_claw_global_per_day`
- after any day reset, `global_state.usdc_claimed_today + usdc_amount <= max_usdc_global_per_day`
- destination `CLAW` token account owner must be the claiming user
- destination `CLAW` token account mint must be `GlobalConfig.claw_mint`
- destination `Test USDC` token account owner must be the claiming user
- destination `Test USDC` token account mint must be `GlobalConfig.usdc_mint`
- faucet vault mints and addresses must match `FaucetConfig` and `GlobalConfig`
- faucet vault balances must be sufficient for the requested transfers

Processing order:

1. read `Clock`
2. compute current `day_index`
3. reset `FaucetGlobalState` counters if its stored day is old
4. initialize or reset `FaucetUserState` counters if its stored day is old
5. validate per-claim, per-wallet, and global limits
6. validate vault and destination token accounts
7. update user and global counters
8. transfer requested `CLAW` amount from `faucet_claw_vault` if `claw_amount > 0`
9. transfer requested `Test USDC` amount from `faucet_usdc_vault` if `usdc_amount > 0`

Counters update before transfers so all state changes remain in one atomic transaction. If a transfer fails, Solana rolls back the full instruction.

## 7. Funding Tooling

Funding uses scripts, not custom chain instructions.

### 7.1 `scripts/phase1/faucet-configure.ts`

Responsibilities:

- initialize faucet accounts
- enable or disable the faucet
- update faucet limits
- read `deployments/devnet-phase1.json`
- verify configured mints and vault addresses match on-chain state

### 7.2 `scripts/phase1/faucet-fund.ts`

Responsibilities:

- fund faucet vaults with `CLAW` or `Test USDC`
- read `deployments/devnet-phase1.json`
- read on-chain `FaucetConfig`
- verify vault mint and authority before moving tokens

Expected modes:

- `--token claw --amount <ui-amount>`
  - transfer existing `CLAW` from the signer/funding wallet's associated token account into `faucet_claw_vault`
- `--token usdc --amount <ui-amount>`
  - if signer is the `testUsdcOperator`, mint `Test USDC` directly into `faucet_usdc_vault`
  - otherwise transfer existing `Test USDC` from the signer/funding wallet's associated token account into `faucet_usdc_vault`

The script must convert UI amounts to base units using `6` decimals.

### 7.3 `scripts/phase1/faucet-status.ts`

Responsibilities:

- show whether the faucet is enabled
- show faucet vault addresses
- show faucet vault balances
- show the current UTC day index
- show global claimed amounts for the current UTC day
- show all six configured limits

## 8. Mainnet Safety

The faucet is a devnet/testnet convenience feature. It must not be enabled on mainnet.

Safety rules:

- `initialize_faucet` creates the faucet disabled by default
- devnet setup must explicitly call `set_faucet_enabled(true)`
- mainnet preflight validation must inspect faucet state before deploy or release sign-off
- validation passes if `FaucetConfig` does not exist
- validation passes if `FaucetConfig.enabled == false`
- validation fails if `FaucetConfig.enabled == true`

The program should not rely on detecting the active Solana cluster on-chain. Cluster-specific policy is enforced through configuration and deployment validation.

## 9. Errors

Add faucet-specific errors:

- `FaucetDisabled`
- `InvalidFaucetAmount`
- `InvalidFaucetLimits`
- `FaucetClaimLimitExceeded`
- `FaucetWalletDailyLimitExceeded`
- `FaucetGlobalDailyLimitExceeded`
- `FaucetVaultInsufficientBalance`
- `InvalidFaucetVault`
- `InvalidFaucetUserState`

Existing token validation errors may be reused where they already describe the failure precisely:

- `InvalidTokenOwner`
- `InvalidTokenMint`
- `InvalidClawMint`
- `InvalidUsdcMint`
- `UnauthorizedAdmin`

## 10. Tests

Program tests should cover:

- faucet initializes with `enabled == false`
- disabled faucet rejects user claims
- admin can enable and disable faucet
- non-admin cannot enable, disable, or update limits
- admin can update valid limits
- invalid limit relationships fail
- user can claim both `CLAW` and `Test USDC` in one instruction
- user can claim only `CLAW` with `usdc_amount == 0`
- user can claim only `Test USDC` with `claw_amount == 0`
- claiming zero for both tokens fails
- per-claim `CLAW` limit is enforced
- per-claim `Test USDC` limit is enforced
- per-wallet UTC-day `CLAW` limit is enforced
- per-wallet UTC-day `Test USDC` limit is enforced
- global UTC-day `CLAW` limit is enforced
- global UTC-day `Test USDC` limit is enforced
- user counters reset after day index changes
- global counters reset after day index changes
- user state is initialized once and reused across claims
- wrong destination owner fails
- wrong destination mint fails
- insufficient faucet vault balance fails

Script tests should cover:

- `faucet-configure.ts` initializes faucet accounts
- `faucet-configure.ts` enables, disables, and updates limits
- `faucet-fund.ts` funds the `CLAW` vault by transfer
- `faucet-fund.ts` funds the `Test USDC` vault by operator mint or transfer
- `faucet-status.ts` reports enabled flag, vault balances, global usage, and limits
- mainnet preflight passes when faucet config is absent
- mainnet preflight passes when faucet config exists but is disabled
- mainnet preflight fails when faucet config exists and is enabled

## 11. Implementation Notes

- Keep faucet logic in a separate instruction module, for example `instructions/faucet.rs`.
- Keep faucet state in the existing state module but separate structs from Phase 1 settlement state.
- Add explicit space constants for each new account.
- Use checked arithmetic for every counter addition.
- Use existing `pool_authority` PDA signer seeds for vault transfers.
- Do not modify `GlobalConfig` account layout for faucet fields.
- Keep `CLAW` and `Test USDC` amounts in base units throughout the program.
- Scripts may accept UI amounts but must convert to base units with `6` decimals.
