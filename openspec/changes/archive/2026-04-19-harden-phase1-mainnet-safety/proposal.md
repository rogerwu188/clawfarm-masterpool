## Why

The current Phase 1 contracts implement the intended receipt and challenge flows, but they are not safe enough for public mainnet deployment. The review found launch-time config takeover risk, stake bypass after provider exit, incomplete reward reversal on successful challenges, a challenged-receipt liveness failure, and unsafe numeric parameter handling; these need to be fixed before the contracts can be treated as production-ready.

## What Changes

- Restrict `masterpool` and `attestation` singleton initialization so an arbitrary first caller cannot seize control during deployment.
- Require providers to remain in `Active` status to receive new receipt settlements, and define the intended behavior for exited providers explicitly.
- Rework successful challenge settlement so reward accounting is unwound at the receipt level instead of socializing losses to the global reward inventory.
- Add a minimal timeout fallback for challenged receipts so funds and provider exits cannot remain blocked forever if the challenge resolver service is unavailable.
- Harden config validation and accounting bounds so split invariants, slash amounts, bond amounts, and signed reward-position math cannot overflow or wrap through admin-configurable values.
- Expand integration and boundary tests to cover deployment authorization, exited-provider rejection, successful-challenge reward reversal, timeout fallback, and unsafe-parameter rejection.

## Capabilities

### New Capabilities
- `secure-program-initialization`: Gate singleton initialization for both programs so only authorized deployment identities can bind governance and linked-program configuration.
- `provider-activity-enforcement`: Reject receipt settlement for providers that are no longer active and preserve the staking invariant after exit.
- `receipt-reward-reversal`: Reverse receipt-booked user and provider rewards when a challenge succeeds, and keep slash economics localized to the challenged receipt instead of charging communal inventory.
- `challenge-timeout-fallback`: Provide a minimal liveness fallback that lets challenged receipts converge after a bounded timeout if the normal resolver path does not execute.
- `bounded-economic-parameters`: Enforce overflow-safe validation for split ratios and economic parameters, and prevent signed accounting state from wrapping under extreme admin inputs.

### Modified Capabilities

None.

## Impact

- Affected programs: `programs/clawfarm-masterpool`, `programs/clawfarm-attestation`
- Affected instruction modules: `src/instructions/config.rs`, `src/instructions/reward.rs`, `src/instructions/receipt.rs`, `src/instructions/challenge.rs`, `src/instructions/provider.rs`, `src/instructions/admin.rs`
- Affected state and validation logic: `src/state/accounts.rs`, `src/state/types.rs`, `src/utils.rs`
- Affected integration surface: attestation-to-masterpool CPI contract, receipt/challenge lifecycle semantics, and operational assumptions around the challenge resolver
- Affected tests: `tests/phase1-integration.ts` plus new unit and integration coverage for authorization, timeout convergence, and numeric bounds
