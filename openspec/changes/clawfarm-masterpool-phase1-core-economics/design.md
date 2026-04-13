## Context

`clawfarm-masterpool` currently exposes an epoch-settlement reward model with a small config account, a single reward vault, and distribution-oriented instructions. The approved Phase 1 business rules require a different architecture: receipt-based economic settlement, provider staking, escrowed USDC provider revenue, challenge-bond accounting in `CLAW`, and locked reward bookkeeping for both users and providers. The repository also already contains a dedicated `clawfarm-attestation` program whose receipt lifecycle reaches `Finalized`, `Rejected`, and `Slashed`, so the new masterpool design must integrate with that program rather than duplicate receipt validation logic.

The design is constrained by three requirements. First, the Phase 1 rules must snapshot economics at receipt or challenge time so later config updates never rewrite history. Second, provider revenue cannot be paid immediately; it must remain escrowed until the attestation program emits a valid final-recognition path. Third, Phase 1 stops short of implementing the daily unlock executor, so reward accounts must define lock and claim accounting without depending on automated daily settlement yet.

## Goals / Non-Goals

**Goals:**
- Replace epoch-based settlement assumptions with receipt-driven economic settlement controlled by the configured attestation program.
- Introduce account models and vaults that make provider stake, pending provider USDC, treasury USDC, reward inventory, and challenge-bond inventory auditable and isolated.
- Ensure receipt and challenge flows are idempotent, parameter-snapshotted, and safe against double payout or post-facto config changes.
- Support provider debt semantics where a negative `CLAW` net position is paid down before new provider rewards become locked rewards.
- Define a Phase 1-compatible claim surface for already released rewards while preserving the Phase 2 unlock executor boundary.

**Non-Goals:**
- Implement the bot or scheduler that materializes daily unlocks.
- Support multi-wallet providers, stake slashing, or generalized off-chain reconciliation.
- Preserve backward compatibility with the existing epoch-settlement ABI.
- Rebuild full receipt or challenge storage inside masterpool; attestation remains the source of truth for dispute state.

## Decisions

### 1. Replace the current settlement core with a receipt-state-machine model

The current `EpochSettlement` flow does not fit receipt-bound economics, payer refunds, or challenge resolution. Phase 1 should introduce new PDAs for `GlobalConfig`, `ProviderAccount`, `ProviderRewardAccount`, `UserRewardAccount`, `ReceiptSettlement`, and `ChallengeBondRecord`, and retire or gate the epoch-distribution instruction path from the active design.

Alternative considered:
- Extend `EpochSettlement` with per-receipt child records. Rejected because it keeps an unnecessary epoch dependency and makes dispute-driven reversals harder to reason about.

### 2. Use masterpool-owned vault separation by economic purpose

The implementation should keep separate token accounts for reward `CLAW`, challenge-bond `CLAW`, treasury USDC, provider stake USDC, and provider-pending USDC. This increases account count slightly, but it keeps accounting simpler and prevents challenge collateral, reward inventory, and settlement cashflows from being conflated.

Alternative considered:
- Share one `CLAW` vault and one USDC vault across all flows. Rejected because it weakens auditability and makes it harder to prove that a burn, refund, stake return, or reward claim used the correct inventory source.

### 3. Snapshot all economic parameters at the point of obligation creation

`record_mining_from_receipt` should compute and store the full receipt settlement snapshot, including payer, provider, USDC split, and `CLAW` reward split. `record_challenge_bond` should snapshot the slash amount and challenger-vs-burn ratio used later by `resolve_challenge_economics`. This avoids history drift when governance changes exchange rates, split ratios, or slash amounts for future activity.

Alternative considered:
- Recompute settlement outcomes from current config during finalize or challenge resolution. Rejected because it violates the explicit business rule that historical receipts and challenges must never be re-derived from current config.

### 4. Keep attestation as the lifecycle authority and masterpool as the economic authority

Attestation already owns receipt and challenge status transitions, so masterpool should trust only CPI calls originating from the configured `attestation_program`. The attestation program must invoke masterpool during four moments: receipt recording, challenge-bond recording, finalized settlement, and challenge-economics resolution. Masterpool should verify the CPI caller, the referenced receipt or challenge identity, and the terminal status required for the requested economic action.

Alternative considered:
- Let off-chain bots call masterpool directly after observing attestation events. Rejected because it creates replay and impersonation risk, and it makes atomic correctness between attestation state transitions and economic effects harder to enforce.

### 5. Model provider penalty as a signed net position instead of immediate stake seizure

Phase 1 explicitly allows negative provider `CLAW` accounting but does not slash the provider's USDC stake directly. `ProviderAccount.claw_net_position` should therefore be a signed integer. Successful challenges reduce this position by the configured slash amount. When a later receipt would grant provider reward `CLAW`, the program first offsets any negative balance and only sends the residual positive amount into `ProviderRewardAccount.locked_claw_total`.

Alternative considered:
- Require providers to prefund or immediately transfer slash `CLAW` on successful challenge. Rejected because it adds extra liquidity requirements and is inconsistent with the confirmed single-net-position debt semantics.

### 6. Represent lockup accounting as aggregate balances, not per-day release events

Phase 1 needs lock accounting but not the scheduler. The design should keep `locked_claw_total`, `released_claw_total`, and `claimed_claw_total` on user/provider reward accounts and treat `released_claw_total` as the amount that has already been materialized by a future executor or admin-safe migration path. `claim_released_claw` transfers only the difference between released and claimed totals and never tries to compute time-based release inline.

Alternative considered:
- Encode day-by-day vesting state in Phase 1 and compute releasable amounts on claim. Rejected because it drifts into Phase 2 executor scope and adds storage and compute complexity before the scheduler design is finalized.

### 7. Treat successful challenge payouts as two distinct flows

When a challenge succeeds, the payer user receives a refund of only the provider-share USDC for the affected receipt, while the challenger receives the configured reward portion of the provider slash in `CLAW` and the remainder is burned. The provider-share USDC refund must come from pending provider escrow and therefore is valid only if the receipt was still in the `Recorded` state; once a receipt is `FinalizedSettled`, it must never transition into a refunded state.

Alternative considered:
- Refund the full receipt amount, including treasury share, or allow reversal after final payout. Rejected because the business rules explicitly retain treasury share and forbid a paid-out finalized receipt from being re-opened economically.

## Risks / Trade-offs

- [Cross-program mismatch] The current attestation implementation still escrows challenge bonds in lamports, while Phase 1 requires `CLAW` bonds managed by masterpool -> Mitigation: include attestation CPI and account-model updates in this change, and make masterpool bond recording the only supported challenge-collateral path.
- [Migration churn] The existing epoch-settlement instruction surface may become obsolete -> Mitigation: gate or remove it in a clearly scoped refactor and update tests and docs together so no mixed-mode behavior remains.
- [State-machine bugs] Double finalization, double refund, or double bond processing would be economically fatal -> Mitigation: enforce explicit enum states, per-receipt/per-challenge idempotency PDAs, and negative tests for every invalid transition.
- [Unlock ambiguity] Reward claimability depends on `released_claw_total`, but Phase 1 does not produce daily releases on its own -> Mitigation: document that Phase 1 only defines storage and claim semantics, and keep any release-materialization instruction out of scope unless it is explicitly specified later.
- [Provider debt UX] Negative `claw_net_position` can block exits for long periods -> Mitigation: expose the net position and unsettled counters directly in provider state so frontends and bots can explain why exit is blocked.

## Migration Plan

1. Add the new state accounts, vault bindings, and instruction modules inside `clawfarm-masterpool`.
2. Update `clawfarm-attestation` CPI flows so receipt submit, challenge open, receipt finalization, and challenge resolution invoke the corresponding masterpool economic instruction.
3. Replace or disable the old epoch-settlement flows in program entrypoints and tests so the repository has one active economic model.
4. Deploy the updated attestation and masterpool programs with matching configured program IDs and vaults.
5. Initialize the new masterpool config, mint the one-time reward supply, register the vaults, and seed provider stake/reward accounts as providers onboard.
6. Roll back by halting new receipt settlement through admin pause flags if any invariant or CPI mismatch is detected before production usage ramps up.

## Open Questions

- Whether challenger reward from provider slash should be transferred immediately during challenge resolution or first counted as released reward inventory for separate claiming. The current design assumes immediate transfer because the design spec describes it as a direct split result.
- Whether pause controls should be a single global switch or separate flags for receipt recording, finalization, challenge economics, and claims. The change can proceed with fine-grained flags if implementation cost stays reasonable.
