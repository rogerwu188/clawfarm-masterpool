## Context

The current Phase 1 programs are close to the intended receipt-driven model, but the review exposed four classes of production risk:

- bootstrap takeover: either singleton config can be initialized by an arbitrary first caller
- broken staking invariant: exited providers can still receive new receipts
- incorrect economic rollback: successful challenges do not unwind receipt-booked rewards
- liveness failure: a challenged receipt can remain blocked forever if the normal resolver path never runs
- unsafe arithmetic: config validation and signed reward accounting rely on overflow-prone operations

The fix must preserve the existing two-program architecture. `clawfarm-attestation` remains the lifecycle authority for receipts and challenges, while `clawfarm-masterpool` remains the economic authority for tokens and accounting. The timeout fallback should be minimal: it should unblock stale challenged receipts without redesigning the full dispute process or introducing a permissionless arbitration system.

## Goals / Non-Goals

**Goals:**
- Prevent unauthorized first-call initialization for both programs.
- Preserve the provider stake invariant by rejecting new economics for non-active providers.
- Make successful challenges economically local to the challenged receipt by unwinding provisional rewards before slash is applied.
- Add a bounded fallback path so stale challenged receipts can converge and provider exits cannot remain blocked indefinitely.
- Replace overflow-prone validation and signed accounting with non-wrapping logic.

**Non-Goals:**
- Introduce a new permissionless dispute court or replace the existing challenge resolver role.
- Add provider re-entry or provider-wallet migration in this change.
- Redesign the release scheduler for locked rewards.
- Support rollback of already-claimed rewards from receipts that were incorrectly released under older semantics.

## Decisions

### 1. Gate initialization with the current program upgrade authority

Both singleton initialization flows will require a bootstrap signer that matches the current ProgramData upgrade authority of the program being initialized. The bootstrap signer may set the long-lived admin, pause, and resolver roles, but it is not itself persisted as the operational governance authority.

This closes the launch-time frontrun gap without hardcoding a deployment pubkey in program code and keeps initialization aligned with the party that can already deploy or upgrade the binary.

Alternatives considered:
- Hardcode a bootstrap pubkey in the program. Rejected because it is inflexible across environments and requires a rebuild for authority rotation.
- Rely on out-of-band “initialize immediately after deploy” process guarantees. Rejected because it does not make the protocol self-defending on public mainnet.

### 2. Treat `Exited` as a terminal provider state in Phase 1

Receipt recording will require `provider.status == Active`. The current provider PDA remains terminal after exit; this change does not add a reactivation or re-staking flow.

This is the smallest safe fix for the stake-bypass finding. It preserves the current one-wallet, one-provider PDA model and avoids introducing new lifecycle edge cases while closing the direct economic loophole.

Alternatives considered:
- Reuse the same provider PDA for re-entry. Rejected because it needs additional state resets, explicit reactivation semantics, and new tests that are not required to close the current bug.
- Allow exited providers to keep earning until a later cleanup. Rejected because it defeats the collateral model entirely.

### 3. Convert receipt-booked rewards into provisional accounting

Receipt-time reward amounts remain snapshotted at `record_mining_from_receipt`, but they are no longer treated as finalized locked rewards. `RewardAccount` gains a provisional balance field, and receipt recording writes:

- user reward into `user_reward_account.pending_claw_total`
- provider locked portion into `provider_reward_account.pending_claw_total`
- provider provisional net-position effect into the provider aggregate state using the existing receipt snapshot math

`ReceiptSettlement` continues to snapshot:

- `claw_to_user`
- `claw_to_provider_total`
- `claw_provider_debt_offset`
- `claw_to_provider_locked`

This preserves receipt-time economics and ordering semantics while making later rollback exact and local to the challenged receipt.

At finalization of a valid receipt:

- pending user reward is moved from `pending_claw_total` to `locked_claw_total`
- pending provider locked reward is moved from `pending_claw_total` to `locked_claw_total`
- provider-share USDC is released as it is today

At successful challenge resolution:

- pending user reward is removed using the receipt snapshot
- pending provider locked reward is removed using the receipt snapshot
- the provider’s provisional reward delta is unwound by subtracting `claw_to_provider_total`
- the configured slash is then applied
- challenger reward and burn are derived from the slash snapshot, not from communal reward losses created by the invalid receipt

This keeps invalid receipts from leaving claimable rewards behind and avoids socializing the invalid-receipt loss to unrelated users.

Alternatives considered:
- Keep aggregate locked balances and subtract from them on successful challenge. Rejected because aggregate locked balances can contain rewards from many receipts and do not prove whether the challenged receipt’s rewards were still unclaimed.
- Stop recording any reward effect until finalization. Rejected because it breaks the current receipt-order debt-offset semantics unless more provider-side sequencing state is added.

### 4. Add an authority-driven timeout reject path for stale challenges

The minimal liveness fix is a new timeout fallback in `clawfarm-attestation`:

- config gains `challenge_resolution_timeout_seconds`
- each challenge has an implicit timeout deadline of `opened_at + challenge_resolution_timeout_seconds`
- if the normal `challenge_resolver` path has not resolved an open challenge by that deadline, `authority` may call a dedicated timeout instruction

The timeout instruction:

- requires the challenge to still be `Open`
- requires the timeout deadline to have passed
- marks the challenge as `Rejected`
- CPIs into masterpool with the existing rejected-challenge economics path so the bond is burned
- leaves the receipt eligible for the normal finalized-settlement flow

This is intentionally conservative. It does not replace the privileged resolver, and it does not auto-accept or auto-slash on timeout. It simply provides an operator-controlled recovery path that lets the system converge if the normal resolver service is down.

Alternatives considered:
- Make timeout resolution permissionless. Rejected for this change because it adds governance and griefing questions that are not required for the minimum viable fix.
- Auto-finalize challenged receipts without burning or resolving the bond. Rejected because it leaves collateral stranded and splits receipt state from challenge state.

### 5. Move signed provider accounting to a wider non-wrapping domain

`ProviderAccount.claw_net_position` will move from an `i64`-sized domain to a wider signed domain, and all config validation and settlement arithmetic will use checked operations. Split validation must use checked addition rather than raw `u16 + u16`, and config updates must reject any parameter set that could overflow reward, slash, bond, or signed net-position computations under valid receipt flows.

This avoids brittle `u64 as i64` casts and turns unsafe arithmetic into explicit validation failures.

Alternatives considered:
- Keep `i64` and add a few hand-tuned caps. Rejected because it is easy to miss one conversion path and the resulting safety envelope becomes harder to reason about.

## Risks / Trade-offs

- [State layout changes] Reward and provider account structs change size, so pre-existing accounts are not binary-compatible. -> Mitigation: treat this as a redeploy or fresh mainnet rollout change rather than an in-place state migration.
- [More settlement steps] Valid receipts now promote provisional rewards during finalization instead of treating them as finalized at record time. -> Mitigation: keep the promotion inside the existing finalized-settlement path so operators do not need a second independent action.
- [Operational centralization remains] The timeout fallback still depends on `authority`, not a permissionless actor set. -> Mitigation: document this as an explicit operational control and keep it bounded to the smallest possible recovery action.
- [Release tooling assumptions change] Reward release automation must read only finalized locked balances, not provisional balances. -> Mitigation: make `materialize_reward_release` operate solely on `locked_claw_total` and extend tests to prove pending balances cannot be released.
- [Spec drift between programs] Masterpool and attestation must agree on timeout behavior and rejected-challenge semantics. -> Mitigation: cover the timeout path in cross-program integration tests and document it in both specs.

## Migration Plan

1. Add bootstrap-authorization checks to both initialization instructions before any production deployment.
2. Extend state models for provisional reward accounting and wider signed provider accounting.
3. Update receipt recording, finalized settlement, challenge resolution, and reward release logic in `clawfarm-masterpool` to use provisional balances and safe arithmetic.
4. Add the timeout-reject instruction and timeout config field to `clawfarm-attestation`.
5. Update integration tests so they cover authorized initialization, exited-provider rejection, successful-challenge reward reversal, timeout recovery, and arithmetic-bound failures.
6. Roll out by deploying upgraded programs, initializing through the authorized bootstrap signer, and withholding production traffic until the new end-to-end tests pass against the deployed binaries.

## Open Questions

- Whether `challenge_resolution_timeout_seconds` should be immutable after initialization or updateable by governance. The safer default is updateable with governance, but a fixed value reduces policy drift.
- Whether the widened signed provider position should be `i128` directly or represented as separate unsigned credit and debt counters. The current design assumes `i128` because it minimizes behavioral change.
