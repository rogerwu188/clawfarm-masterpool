# Phase 1 Core Economics

This document describes the active on-chain economic model in this repository.

## Account Layout

### `GlobalConfig`

Stores:

- admin authority
- configured attestation program
- `CLAW` and `USDC` mint bindings
- reward, challenge-bond, treasury, provider-stake, and provider-pending vaults
- governance parameters for stake, reward splits, slash amount, bond amount,
  exchange rate, and lock duration
- pause flags for receipt recording, challenge processing, finalization, and
  claims

### `ProviderAccount`

Stores:

- provider wallet identity
- staked USDC amount
- pending provider USDC
- signed `claw_net_position`
- unsettled receipt count
- unresolved challenge count
- active or exited status

### `RewardAccount`

One PDA per user or provider. Stores:

- owner
- account kind (`User` or `Provider`)
- `locked_claw_total`
- `released_claw_total`
- `claimed_claw_total`

Phase 1 includes an admin-only manual release-materialization helper for tests
and controlled migrations. It is not a scheduler.

### `ReceiptSettlement`

One PDA per attestation receipt. Stores the immutable receipt-time economic
snapshot:

- payer user
- provider wallet
- total USDC charged
- treasury-share USDC
- provider-share USDC
- user reward `CLAW`
- provider reward `CLAW`
- provider debt offset amount
- provider locked reward amount
- settlement status

### `ChallengeBondRecord`

One PDA per attestation challenge. Stores:

- receipt and challenge identity
- challenger
- payer user
- provider wallet
- fixed challenge bond amount
- snapshotted provider slash amount
- challenger-reward and burn split snapshots
- precomputed challenger reward and burn amounts
- bond status

## Vaults

Masterpool owns separate PDA token accounts for:

- reward `CLAW`
- challenge-bond `CLAW`
- treasury `USDC`
- provider stake `USDC`
- provider pending-revenue `USDC`

This keeps reward inventory, challenge collateral, provider escrow, and treasury
funds separate for auditing and state-machine safety.

## Instruction Surface

### Masterpool

- `initialize_masterpool`
- `mint_genesis_supply`
- `update_config`
- `set_pause_flags`
- `register_provider`
- `exit_provider`
- `materialize_reward_release`
- `claim_released_claw`
- `record_mining_from_receipt`
- `settle_finalized_receipt`
- `record_challenge_bond`
- `resolve_challenge_economics`

### Attestation

- `initialize_config`
- `upsert_provider_signer`
- `set_pause`
- `revoke_provider_signer`
- `submit_receipt`
- `open_challenge`
- `resolve_challenge`
- `finalize_receipt`
- `close_challenge`
- `close_receipt`

## Receipt Flow

1. A verified attestation receipt is submitted.
2. Attestation CPIs into masterpool `record_mining_from_receipt`.
3. Masterpool charges payer USDC, splits treasury and provider escrow, snapshots
   the receipt settlement, books user rewards, and books provider rewards after
   debt offset.
4. If the receipt later finalizes, attestation CPIs into
   `settle_finalized_receipt`.
5. Masterpool releases the provider-share USDC, decrements pending revenue and
   unsettled count, and marks the settlement finalized.

## Challenge Flow

1. A challenger opens a challenge against a submitted receipt.
2. Attestation CPIs into `record_challenge_bond`.
3. Masterpool transfers the fixed `CLAW` bond into the challenge-bond vault and
   snapshots slash economics.
4. If the challenge is rejected:
   - masterpool burns the bond
   - the receipt stays economically recorded and can still be finalized later
5. If the challenge is accepted:
   - masterpool returns the bond
   - refunds provider-share USDC to the payer user
   - subtracts the provider slash from signed net position
   - transfers the challenger reward portion from reward inventory
   - burns the remainder
   - permanently blocks later provider payout for that receipt

## Removed Behavior

The repository no longer uses the old epoch distribution model for active
testing or documentation:

- no epoch settlement submission
- no epoch-wide reward distribution
- no epoch finalization as the economic authority

All active verification now runs through receipt-driven attestation-to-masterpool
CPI tests.
