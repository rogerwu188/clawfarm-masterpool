# clawfarm-masterpool

`clawfarm-masterpool` is the Solana economic authority for Clawfarm Phase 1.

Chinese version:

- [README.zh-CN.md](README.zh-CN.md)

This README reflects the current implementation in this repository.

Source of truth:

- [src/lib.rs](src/lib.rs)
- [src/instructions/config.rs](src/instructions/config.rs)
- [src/instructions/provider.rs](src/instructions/provider.rs)
- [src/instructions/reward.rs](src/instructions/reward.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [../../docs/phase1-testnet-runbook.md](../../docs/phase1-testnet-runbook.md)

## Responsibilities

- bind the fixed `CLAW` mint and fixed settlement mint in `GlobalConfig`
- own the reward, bond, treasury, provider-stake, and provider-pending vaults
- register providers against a fixed USDC stake
- record receipt-time USDC and `CLAW` economics from attestation CPI calls
- settle finalized provider payout after attestation finalization
- process challenge-bond, slash, reward, and burn economics
- release and claim vested `CLAW`

## Token Binding Model

Phase 1 does not allow arbitrary settlement mints.

`GlobalConfig` binds:

- `claw_mint`
- `usdc_mint`
- five protocol vault addresses

Runtime implications:

- provider registration requires `usdc_mint == config.usdc_mint`
- receipt recording requires `usdc_mint == config.usdc_mint`
- challenge settlement requires `usdc_mint == config.usdc_mint`
- attestation no longer forwards a user-supplied `charge_mint`

## Receipt CPI Interface

Attestation records economics through the compact CPI args:

```rust
pub struct RecordMiningFromReceiptArgs {
    pub total_usdc_paid: u64,
}
```

At record time masterpool:

- verifies the caller is the configured attestation program
- verifies the payer token account owner and mint
- splits payer USDC into treasury share and provider pending share
- snapshots user and provider `CLAW` rewards into pending balances
- creates the `ReceiptSettlement` PDA keyed by attestation receipt

At finalization time masterpool:

- promotes pending rewards into locked balances
- transfers provider-share USDC from provider-pending vault to provider wallet
- marks the settlement `FinalizedSettled`

## Program State

### `GlobalConfig`

Stores:

- admin authority
- configured attestation program id
- bound `CLAW` and `USDC` mint addresses
- reward, bond, treasury, provider-stake, and provider-pending vault addresses
- Phase 1 economics parameters
- one-time genesis mint flag
- pause flags

### `ProviderAccount`

Seed:

- `("provider", provider_wallet)`

Stores:

- provider wallet identity
- staked USDC amount
- pending provider-share USDC
- signed `claw_net_position`
- unsettled receipt count
- unresolved challenge count
- provider status

### `RewardAccount`

Seeds:

- `("user_reward", user_wallet)`
- `("provider_reward", provider_wallet)`

Stores reward totals as:

- `pending_claw_total`
- `locked_claw_total`
- `released_claw_total`
- `claimed_claw_total`

### `ReceiptSettlement`

Seed:

- `("receipt_settlement", attestation_receipt)`

Stores the immutable receipt-time economics snapshot, including:

- payer and provider identities
- total paid USDC
- treasury and provider USDC split
- user and provider `CLAW` reward amounts
- provider debt offset and locked amount
- reward lock timing and release progress
- settlement status

### `ChallengeBondRecord`

Seed:

- `("challenge_bond_record", attestation_challenge)`

Stores the snapshotted challenge economics used to resolve accepted or rejected
challenges.

## Operational Constraints

- both token mints must use `6` decimals
- `initialize_masterpool` requires the current upgrade authority via
  `ProgramData`
- `mint_genesis_supply` mints the one-time `1_000_000_000 * 10^6` `CLAW`
  inventory, then revokes mint and freeze authority
- accepted challenges refund only the provider-share USDC; treasury share stays
  in the treasury vault
- provider exit requires zero pending provider USDC, zero unsettled receipts,
  and zero unresolved challenges
- reward release is a manual admin-triggered vesting materialization step in
  Phase 1
