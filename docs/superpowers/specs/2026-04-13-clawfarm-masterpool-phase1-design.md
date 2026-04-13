# Clawfarm Masterpool Phase 1 Design

Status: Draft for review
Date: 2026-04-13
Scope: Phase 1 core economic rules only
Upstream intent: Designed to be easy to map into OpenSpec `proposal.md`, `design.md`, and `tasks.md`

## 1. Goal

Build a Solana `clawfarm-masterpool` program that acts as the on-chain economic
layer for Clawfarm usage settlement.

Phase 1 focuses on the core accounting and fund-control rules:

- mint a fixed `1_000_000_000` `CLAW` reward supply once into a reward vault
- register Providers behind a USDC stake requirement
- record receipt-driven mining economics only from the attestation program
- split user-paid USDC between Provider revenue and treasury revenue
- award `CLAW` to users and Providers under a lockup model
- escrow Provider revenue until the underlying receipt is fully recognized
- support challenge-bond economics and post-challenge penalties
- support user / Provider claiming of already released rewards later

Phase 1 explicitly does **not** include the daily unlock executor. Unlock timing
rules are part of the accounting model, but the scheduled release mechanism is a
Phase 2 concern.

## 2. Non-Goals

Phase 1 does not attempt to solve:

- daily scheduled release execution
- provider identity abstraction beyond one wallet per provider
- multi-wallet provider management
- dynamic challenge bond sizing
- direct slashing of Provider USDC stake
- generalized off-chain settlement reconciliation
- backward-compatible migration from the current epoch-settlement-based
  `clawfarm-masterpool`

## 3. Actors and Assets

### Actors

- `deployer_admin`: contract deployer / top-level admin; can update config
- `attestation_program`: only trusted CPI caller for receipt-based economic
  actions
- `provider_wallet`: a Provider's sole on-chain identity in Phase 1
- `payer_user`: the actual wallet whose USDC is charged for an LLM call
- `challenger`: the wallet that opens a challenge and posts the challenge bond
- `treasury`: recipient of treasury USDC share and one of the sinks in the
  economic model
- `release_bot` (Phase 2): future executor that materializes daily unlocks

### Assets

- `CLAW`: reward / penalty token minted once, managed by the masterpool program;
  uses `6` decimals
- `USDC`: settlement token for provider stake and user payment flow

## 4. Fixed High-Level Rules Confirmed

The following business rules are fixed for Phase 1.

### Reward pool

- Mint `1,000,000,000` `CLAW` once.
- Mint the entire supply into a masterpool-controlled reward vault.
- The protocol should not rely on long-lived mint authority for routine reward
  issuance.
- `CLAW` uses `6` decimals, so the on-chain mint supply is
  `1_000_000_000 * 10^6`.

### Provider identity and stake

- One Provider equals one primary wallet address.
- Provider registration requires a USDC stake.
- Default stake amount is `100 USDC`.
- The stake is primarily a registration / exit gate in Phase 1.
- A Provider cannot exit while any unresolved economic obligations remain.

### User payment and mining

- The payer user signs the same transaction that pays USDC.
- Mining settlement is receipt-based.
- The core mining settlement entrypoint is callable only by the attestation
  program via CPI.
- Default USDC split is stored in per-thousand units (`bps1000` style):
  - Provider share: `300`
  - Treasury share: `700`
- Default `CLAW` reward split is:
  - user: `300`
  - Provider: `700`

### Lockup model

- Both user rewards and Provider rewards are initially locked.
- Default lock duration is `180` days.
- Rewards unlock linearly by day.
- Only released rewards become claimable.
- The actual daily release executor is postponed to Phase 2.

### Challenge economics

- Opening a challenge requires a real `CLAW` bond from the challenger.
- Challenge bond amount is an independently configurable fixed amount.
- Challenger bond cannot become negative; it must be fully funded.
- If challenge fails:
  - challenger bond is confiscated and burned
- If challenge succeeds:
  - challenger bond is returned
  - Provider receives a `CLAW` penalty
  - penalty is split between challenger reward and burn
- Provider `CLAW` penalty amount is independently configurable.
- Default challenger reward vs burn split is:
  - challenger reward: `700`
  - burn: `300`

### Provider penalty semantics

- Provider `CLAW` accounting uses a single net position.
- Net position can become negative.
- New Provider reward accrual must first pay down negative balance.
- A Provider with unresolved negative `CLAW` position cannot exit.

### Receipt success / failure semantics

- A receipt is economically recognized only when attestation marks it
  `Finalized`.
- `Finalized` includes both:
  - no challenge and challenge window elapsed
  - challenged but challenge rejected
- `Rejected` / `Slashed` receipts are not economically recognized.

### USDC refund semantics on successful challenge

- If challenge succeeds, the actual refund recipient is the receipt-bound payer
  user.
- That payer address must be bound inside attestation receipt data and used by
  masterpool settlement.
- Refund amount is only the Provider share of USDC from that receipt.
- Treasury share is not refunded in Phase 1.

### Provider revenue release semantics

- Provider USDC share is not paid out immediately.
- Provider share is first held in masterpool-controlled pending revenue.
- Only after attestation confirms the receipt as `Finalized` may masterpool
  release Provider USDC.
- This Provider release must not be keyed off attestation `close_receipt`.
- It must be keyed off a dedicated attestation-to-masterpool settlement signal
  that is only valid for `Finalized` receipts.

## 5. Governance Parameters

All parameters below are modifiable only by the deployer / admin authority.

- `exchange_rate_usdc_to_claw`
  - default `1:1`
  - interpreted in base units with both assets using `6` decimals, so
    `1 USDC` maps to `1 CLAW` by default
- `provider_stake_usdc`
  - default `100 USDC`
- `provider_usdc_share_bps1000`
  - default `300`
- `treasury_usdc_share_bps1000`
  - default `700`
- `user_claw_share_bps1000`
  - default `300`
- `provider_claw_share_bps1000`
  - default `700`
- `lock_days`
  - default `180`
- `provider_slash_claw_amount`
  - default equal to `1 USDC` worth of `CLAW` under current exchange rule
- `challenger_reward_bps1000`
  - default `700`
- `burn_bps1000`
  - default `300`
- `challenge_bond_claw_amount`
  - configurable fixed amount

Parameter invariants:

- `provider_usdc_share_bps1000 + treasury_usdc_share_bps1000 == 1000`
- `user_claw_share_bps1000 + provider_claw_share_bps1000 == 1000`
- `challenger_reward_bps1000 + burn_bps1000 == 1000`
- stake / bond / slash values must all be non-zero positive integers
- `CLAW` mint decimals are fixed at `6`
- `USDC` is assumed to use `6` decimals in Phase 1 accounting
- parameter changes apply only to future receipts / future challenges
- historical receipt settlements must use snapshotted values, never re-derived
  from the latest config

## 6. Account Model

Phase 1 should use a single-program total-accounting model.

### 6.1 `GlobalConfig`

Purpose:

- singleton economic config and address registry

Suggested fields:

- `admin_authority: Pubkey`
- `attestation_program: Pubkey`
- `claw_mint: Pubkey`
- `usdc_mint: Pubkey`
- `reward_vault: Pubkey`
- `treasury_usdc_vault: Pubkey`
- `provider_stake_vault: Pubkey`
- `provider_pending_usdc_vault: Pubkey` or equivalent vault binding
- `challenge_bond_vault: Pubkey`
- all governance parameters listed above
- optional pause flags

### 6.2 `ProviderAccount`

One PDA per `provider_wallet`.

Purpose:

- provider registration state and aggregate financial position

Suggested fields:

- `provider_wallet: Pubkey`
- `staked_usdc_amount: u64`
- `pending_provider_usdc: u64`
- `claw_net_position: i128` or signed balance large enough for debt semantics
- `unsettled_receipt_count: u64`
- `status: Active | Exiting | Frozen`

Rules:

- `claw_net_position` may be negative
- `pending_provider_usdc` is positive-only
- provider cannot exit unless all blocking conditions are cleared

### 6.3 `ProviderRewardAccount`

One PDA per `provider_wallet`.

Purpose:

- Provider lockup accounting

Suggested fields:

- `locked_claw_total: u64`
- `released_claw_total: u64`
- `claimed_claw_total: u64`

Rule:

- if `ProviderAccount.claw_net_position < 0`, new Provider reward accrual first
  reduces debt; only any remaining positive portion is added into
  `locked_claw_total`

### 6.4 `UserRewardAccount`

One PDA per `payer_user`.

Purpose:

- user reward lockup accounting

Suggested fields:

- `locked_claw_total: u64`
- `released_claw_total: u64`
- `claimed_claw_total: u64`

### 6.5 `ReceiptSettlement`

One PDA per attestation `receipt` pubkey.

Purpose:

- per-receipt economic snapshot and idempotency anchor

Suggested fields:

- `attestation_receipt: Pubkey`
- `payer_user: Pubkey`
- `provider_wallet: Pubkey`
- `usdc_total_paid: u64`
- `usdc_to_provider: u64`
- `usdc_to_treasury: u64`
- `claw_to_user: u64`
- `claw_to_provider: u64`
- `status: Recorded | FinalizedSettled | ChallengedReverted`

Notes:

- this account must snapshot the exact economic result of the receipt under the
  config values in effect at record time
- later challenge or finalize flows must use this snapshot, not current config

### 6.6 `ChallengeBondRecord`

One PDA per attestation `challenge` pubkey.

Purpose:

- challenge bond accounting and idempotency anchor

Suggested fields:

- `attestation_challenge: Pubkey`
- `challenger: Pubkey`
- `bond_amount: u64`
- `slash_claw_amount_snapshot: u64`
- `challenger_reward_bps1000_snapshot: u16`
- `burn_bps1000_snapshot: u16`
- `status: Locked | Returned | Burned`

### 6.7 Vault Layer

Suggested managed vaults:

- `reward_claw_vault`
- `challenge_bond_claw_vault`
- `treasury_usdc_vault`
- `provider_stake_usdc_vault`
- `provider_pending_usdc_vault` or equivalent escrow layout

Design preference:

- keep vault count as low as possible, but do not mix conceptually distinct
  assets if it weakens accounting clarity
- reward inventory and challenger bond inventory should stay separate even if
  both are `CLAW`

## 7. Core Instruction Set

### 7.1 Admin

#### `initialize_masterpool`

Responsibilities:

- initialize `GlobalConfig`
- create or bind core vaults
- mint `1_000_000_000 CLAW` into reward vault
- configure `attestation_program`

#### `update_config`

Responsibilities:

- update economic parameters
- enforce ratio invariants
- never mutate past receipt settlements

### 7.2 Provider Lifecycle

#### `register_provider`

Caller:

- `provider_wallet`

Responsibilities:

- transfer Provider stake USDC into stake vault
- initialize `ProviderAccount`
- initialize `ProviderRewardAccount`

#### `exit_provider`

Caller:

- `provider_wallet`

Allowed only if:

- `pending_provider_usdc == 0`
- `unsettled_receipt_count == 0`
- `claw_net_position >= 0`
- no unresolved challenge economics remain

Responsibilities:

- return Provider stake
- mark provider inactive / exited

### 7.3 Attestation-Only CPI Entrypoints

These instruction paths must only accept CPI calls from the configured
`attestation_program`.

#### `record_mining_from_receipt`

Triggered by:

- attestation receipt submission flow after successful `submit_receipt`

Required logical inputs:

- `attestation_receipt`
- `payer_user`
- `provider_wallet`
- `usdc_total_paid`
- enough receipt identity data to guarantee uniqueness and consistency

Responsibilities:

1. charge USDC from the `payer_user`
2. split USDC into Provider portion and treasury portion
3. transfer treasury portion into treasury vault
4. escrow Provider portion into pending Provider revenue accounting
5. compute `CLAW` reward amounts from the current exchange ratio
6. credit user locked rewards
7. apply Provider reward to debt first, then locked rewards
8. create `ReceiptSettlement(status = Recorded)`
9. increment Provider unsettled receipt tracking as needed

#### `record_challenge_bond`

Triggered by:

- attestation challenge-open flow

Responsibilities:

1. transfer fixed `CLAW` challenge bond from challenger into challenge bond vault
2. create `ChallengeBondRecord(status = Locked)`

#### `settle_finalized_receipt`

Triggered by:

- attestation-controlled settlement signal for a `Finalized` receipt

Responsibilities:

1. verify the attestation receipt is in final recognized state
2. verify `ReceiptSettlement.status == Recorded`
3. release `usdc_to_provider` from escrow to Provider wallet
4. reduce `pending_provider_usdc`
5. mark settlement `FinalizedSettled`
6. decrement Provider unsettled receipt tracking as needed

Important rule:

- this instruction must not be callable for `Rejected` or `Slashed` receipts
- this instruction is distinct from attestation account cleanup

#### `resolve_challenge_economics`

Triggered by:

- attestation challenge resolution flow

Path A: challenge rejected

1. burn challenger bond from challenge bond vault
2. mark `ChallengeBondRecord = Burned`
3. keep receipt settlement in `Recorded` so it may later be finalized and paid

Path B: challenge accepted / receipt invalidated / signer revoked

1. return challenger bond
2. reduce Provider `claw_net_position` by `provider_slash_claw_amount`
3. split slash result into challenger reward and burn using the ratio snapshotted at challenge-open time
4. refund the receipt's `usdc_to_provider` portion to `payer_user`
5. mark `ReceiptSettlement = ChallengedReverted`
6. block any future Provider payout for that receipt
7. decrement Provider unsettled receipt tracking as needed

### 7.4 User / Provider Claim

#### `claim_released_claw`

Caller:

- owner of `UserRewardAccount` or `ProviderRewardAccount`

Responsibilities:

- transfer `released_claw_total - claimed_claw_total`
- increase `claimed_claw_total`

Note:

- Phase 1 defines the claim path but not the daily unlock executor

## 8. State Flow

### 8.1 Normal recognized receipt

1. user signs transaction
2. attestation verifies and records receipt
3. attestation CPI calls `record_mining_from_receipt`
4. Provider USDC share becomes escrowed pending revenue
5. receipt later reaches attestation `Finalized`
6. attestation CPI calls `settle_finalized_receipt`
7. Provider receives the escrowed USDC

### 8.2 Challenge rejected

1. challenger posts `CLAW` bond
2. challenge is resolved as rejected
3. challenger bond is burned
4. receipt remains eligible for recognized final settlement
5. Provider may still get escrowed USDC after attestation final-recognition

### 8.3 Challenge accepted

1. challenger posts `CLAW` bond
2. challenge is resolved as accepted / invalidating
3. challenger bond is returned
4. Provider receives a `CLAW` penalty by net-position reduction
5. challenger receives reward portion of slash
6. burn portion is destroyed
7. receipt-bound `payer_user` gets Provider-share USDC refund
8. receipt is permanently blocked from Provider payout path

## 9. Security Invariants

The design must preserve these invariants.

### Call-source invariants

- only configured `attestation_program` may invoke receipt-settlement CPI
  endpoints
- external users must not be able to directly fabricate mining, refund, or slash
  events

### Idempotency invariants

- a given attestation receipt may be economically recorded exactly once
- a given attestation receipt may be Provider-settled exactly once
- a given attestation challenge bond may be recorded exactly once
- a given attestation challenge economics result may be processed exactly once

### Mutual exclusion invariants

- `ReceiptSettlement.FinalizedSettled` can never transition to refunded /
  reverted state
- `ReceiptSettlement.ChallengedReverted` can never transition to Provider payout
  state

### Accounting invariants

- snapshotted per-receipt amounts must sum exactly to original paid amount and
  original reward amount
- config changes must never retroactively mutate economic results of historical
  receipts
- Provider `claw_net_position` may go negative, but challenge bond balance may
  not

### Exit invariants

- Provider stake cannot be withdrawn while any unsettled receipt, pending
  Provider revenue, unresolved challenge economics, or negative `CLAW` net
  position exists

## 10. Missing But Recommended Safety Controls

The following are not explicitly required in the original feature list but are
strongly recommended.

### Module pause flags

Prefer separate pause flags instead of one global kill switch:

- `pause_provider_registration`
- `pause_receipt_recording`
- `pause_claim`
- `pause_challenge_economics`

### Explicit status enums

Keep status models narrow and machine-checkable:

- `ProviderStatus`
- `ReceiptSettlementStatus`
- `ChallengeBondStatus`

### Snapshot-first design

All values that affect later settlement should be snapshotted into
`ReceiptSettlement` at record time.

## 11. Open Implementation Decisions To Freeze In Planning

These are not blockers for the design, but they must be frozen before code is
written.

- exact PDA seed layout for each account
- whether Provider pending USDC uses one shared vault plus accounting, or
  provider-specific escrow vaults
- whether challenger reward from Provider slash is sent immediately or first
  enters a lockup account
- exact signed integer width for `claw_net_position`
- exact storage format for `exchange_rate_usdc_to_claw`; recommended default is
  rational form with numerator / denominator in 6-decimal base units so the
  initial ratio is exactly `1_000_000 : 1_000_000`

## 12. Phase 2 Boundary

Phase 2 should add:

- daily unlock executor
- precise unlock schedule materialization logic
- bot retry / catch-up semantics
- possibly Provider stake slashing or more advanced recovery logic

Phase 1 should not depend on Phase 2 automation to preserve correctness of core
receipt economics.

## 13. OpenSpec Mapping Guidance

This document is intentionally structured so it can be reused in OpenSpec.

Recommended mapping:

- `proposal.md`
  - sections 1, 2, 3, 4
- `design.md`
  - sections 5, 6, 7, 8, 9, 10, 11, 12
- `tasks.md`
  - derive from sections 6, 7, 9, 10, 11
