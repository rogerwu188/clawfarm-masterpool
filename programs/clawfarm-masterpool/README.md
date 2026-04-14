# clawfarm-masterpool

`clawfarm-masterpool` is the Solana economic authority for Clawfarm Phase 1.

Chinese version:

- [README.zh-CN.md](README.zh-CN.md)

This README documents the current on-chain implementation in this repository.

Source of truth:

- [src/lib.rs](src/lib.rs)
- [src/instructions/config.rs](src/instructions/config.rs)
- [src/instructions/provider.rs](src/instructions/provider.rs)
- [src/instructions/reward.rs](src/instructions/reward.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [../../docs/phase1-core-economics.md](../../docs/phase1-core-economics.md)
- [../../tests/phase1-integration.ts](../../tests/phase1-integration.ts)

## Responsibilities

- own and operate the reward `CLAW`, challenge-bond `CLAW`, treasury `USDC`,
  provider-stake `USDC`, and provider-pending `USDC` vaults
- register providers against a fixed USDC stake and allow exit only after all
  obligations are cleared
- record receipt-time USDC and `CLAW` economics from attestation CPI calls
- keep user and provider reward balances as locked, released, and claimed
  totals
- release provider pending USDC only after attestation finalization
- settle challenge-bond and provider-slash economics after attestation
  resolution
- mint the one-time genesis `CLAW` inventory into the reward vault

## High-Level Model

Phase 1 is receipt-driven, not epoch-driven.

The active trust split is:

- `clawfarm-attestation` owns receipt and challenge lifecycle state
- `clawfarm-masterpool` owns all token movement and economic state

At receipt record time:

- payer USDC is split immediately between treasury and provider pending revenue
- user and provider `CLAW` rewards are snapshotted immediately
- provider negative `claw_net_position` is paid down before new locked provider
  rewards are added

At receipt finalization time:

- provider-share USDC is released from the pending vault to the provider wallet

At challenge resolution time:

- rejected challenges burn the challenger bond
- accepted challenges return the bond, refund provider-share USDC to the payer,
  slash the provider signed `CLAW` position, transfer the challenger reward
  from reward inventory, and burn the remainder

## Current Implementation Constraints

- both Phase 1 token mints must use `6` decimals
- basis-point splits use a `1000` scale and are validated on config updates
- the fixed genesis mint can only run once through `mint_genesis_supply`
- `mint_genesis_supply` mints `1_000_000_000 * 10^6` `CLAW` and then revokes
  mint and freeze authority
- one `ProviderAccount` exists per provider wallet
- one `ReceiptSettlement` exists per attestation receipt
- one `ChallengeBondRecord` exists per attestation challenge
- attestation-only entrypoints require the attestation config PDA from the
  configured attestation program to be the signer
- accepted challenges refund only the provider-share USDC; the treasury share
  stays in the treasury vault
- reward release is currently an admin-only manual helper, not an automated
  unlock scheduler

## Governance Parameters

`Phase1ConfigParams` currently defines:

- `exchange_rate_claw_per_usdc_e6`
- `provider_stake_usdc`
- `provider_usdc_share_bps`
- `treasury_usdc_share_bps`
- `user_claw_share_bps`
- `provider_claw_share_bps`
- `lock_days`
- `provider_slash_claw_amount`
- `challenger_reward_bps`
- `burn_bps`
- `challenge_bond_claw_amount`

## Program State

State definitions live in [src/state/accounts.rs](src/state/accounts.rs).

### `GlobalConfig`

PDA seed:

- `["config"]`

Stores:

- admin authority
- configured attestation program id
- bound `CLAW` and `USDC` mint addresses
- five protocol vault addresses
- Phase 1 governance parameters
- one-time genesis mint flag
- pause flags for receipt recording, challenge processing, finalization, and
  claims

### `ProviderAccount`

PDA seed:

- `["provider", provider_wallet]`

Stores:

- provider wallet identity
- staked USDC amount
- pending provider-share USDC
- signed `claw_net_position`
- unsettled receipt count
- unresolved challenge count
- provider status

### `RewardAccount`

PDA seeds:

- `["user_reward", user_wallet]`
- `["provider_reward", provider_wallet]`

Stores:

- owner
- account kind (`User` or `Provider`)
- `locked_claw_total`
- `released_claw_total`
- `claimed_claw_total`

### `ReceiptSettlement`

PDA seed:

- `["receipt_settlement", attestation_receipt]`

Stores the immutable receipt-time economic snapshot:

- payer user
- provider wallet
- total USDC paid
- treasury-share USDC
- provider-share USDC
- user reward `CLAW`
- provider reward `CLAW`
- provider debt offset amount
- provider locked reward amount
- settlement status

### `ChallengeBondRecord`

PDA seed:

- `["challenge_bond_record", attestation_challenge]`

Stores the snapshotted challenge economics:

- attestation receipt and challenge identities
- challenger, payer, and provider identities
- fixed bond amount
- provider slash snapshot
- challenger reward and burn basis points
- precomputed challenger reward and burn amounts
- bond status

## Status Values

Definitions live in [src/state/types.rs](src/state/types.rs).

### `ProviderStatus`

- `0 = Active`
- `1 = Exited`

### `RewardAccountKind`

- `0 = User`
- `1 = Provider`

### `ReceiptSettlementStatus`

- `0 = Recorded`
- `1 = FinalizedSettled`
- `2 = ChallengedReverted`

### `ChallengeBondStatus`

- `0 = Locked`
- `1 = Returned`
- `2 = Burned`

## Vault Layout

Masterpool owns separate PDA token accounts for:

- `["reward_vault"]` for reward `CLAW`
- `["challenge_bond_vault"]` for challenge-bond `CLAW`
- `["treasury_usdc_vault"]` for treasury `USDC`
- `["provider_stake_usdc_vault"]` for provider stake `USDC`
- `["provider_pending_usdc_vault"]` for provider pending revenue `USDC`

The shared vault authority PDA is:

- `["pool_authority"]`

This separation keeps reward inventory, challenge collateral, provider escrow,
and treasury balances auditable and prevents state-machine overlap.

## Instruction Surface

Entry points live in [src/lib.rs](src/lib.rs).

### Admin

- `initialize_masterpool(params: Phase1ConfigParams)`
  - creates `GlobalConfig` and all vault PDAs
  - binds the attestation program and both token mints
  - validates decimals and governance invariants
- `mint_genesis_supply()`
  - mints the fixed genesis supply into the reward vault
  - revokes mint and freeze authority on the `CLAW` mint
- `update_config(params: Phase1ConfigParams)`
  - updates the active Phase 1 economic parameters
- `set_pause_flags(pause_receipt_recording, pause_challenge_processing, pause_finalization, pause_claims)`
  - toggles the four runtime circuit breakers

### Provider

- `register_provider()`
  - transfers the fixed USDC stake from the provider wallet
  - creates the provider account and provider reward account
- `exit_provider()`
  - returns the provider stake only when pending USDC, unsettled receipts,
    unresolved challenges, and negative `claw_net_position` are all cleared

### Reward

- `materialize_reward_release(amount)`
  - admin-only helper that moves `amount` from locked to released
- `claim_released_claw()`
  - transfers currently claimable `CLAW` from the reward vault to the owner

### Attestation-only Economic Hooks

- `record_mining_from_receipt(args: { total_usdc_paid, charge_mint })`
  - validates the attestation caller
  - charges the payer in USDC
  - splits treasury and provider-share USDC
  - initializes reward accounts when needed
  - books user rewards and provider rewards after debt offset
  - creates the immutable `ReceiptSettlement`
- `settle_finalized_receipt(attestation_receipt_status)`
  - validates that attestation finalized the receipt
  - releases provider-share USDC from the pending vault to the provider wallet
  - marks the settlement `FinalizedSettled`
- `record_challenge_bond()`
  - transfers the fixed `CLAW` bond from the challenger into the bond vault
  - snapshots the slash, challenger reward, and burn economics
- `resolve_challenge_economics(resolution_code)`
  - burns the bond on rejected challenges
  - or returns the bond, refunds provider-share USDC, updates
    `claw_net_position`, pays the challenger reward, and burns the remainder on
    accepted challenge outcomes

## Receipt Settlement Flow

1. `clawfarm-attestation::submit_receipt` verifies the canonical receipt and
   CPIs into `record_mining_from_receipt`.
2. Masterpool transfers the payer USDC into the treasury vault and provider
   pending vault.
3. Masterpool books user `CLAW` rewards and provider `CLAW` rewards, first
   offsetting any negative provider `claw_net_position`.
4. Masterpool creates a `ReceiptSettlement` snapshot keyed by the attestation
   receipt PDA.
5. When attestation later finalizes the receipt, it CPIs into
   `settle_finalized_receipt`.
6. Masterpool releases the provider-share USDC and marks the settlement
   finalized.

## Challenge Flow

1. `clawfarm-attestation::open_challenge` CPIs into `record_challenge_bond`.
2. Masterpool locks the fixed `CLAW` bond and snapshots challenge economics.
3. `clawfarm-attestation::resolve_challenge` later CPIs into
   `resolve_challenge_economics`.
4. If the challenge is rejected:
   - the challenger bond is burned
   - the receipt stays economically recorded
   - attestation can still finalize the receipt later for provider payout
5. If the challenge is accepted, invalidated, or signer-revoked:
   - the challenger bond is returned
   - provider-share USDC is refunded to the payer
   - the provider signed `CLAW` position is slashed
   - the challenger reward is paid from reward inventory
   - the remaining slash amount is burned
   - the receipt settlement is marked `ChallengedReverted`

## Tested Behavior

The current end-to-end integration test in
[../../tests/phase1-integration.ts](../../tests/phase1-integration.ts) covers:

- provider registration
- unauthorized direct `record_mining_from_receipt` failure
- normal receipt recording and finalization
- duplicate receipt prevention
- rejected challenge burn path plus later finalization
- accepted challenge refund and slash path
- user reward release and claim
- provider reward release and claim
- provider exit blocked until all obligations are cleared

## Development

```bash
anchor build
anchor test
```
