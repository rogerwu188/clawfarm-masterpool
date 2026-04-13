## Why

The current `clawfarm-masterpool` program still reflects an epoch-settlement reward model, while the Phase 1 business rules now require receipt-driven accounting, provider staking, escrowed USDC settlement, challenge-bond economics, and locked `CLAW` rewards. This change is needed now to align the on-chain program with the attestation-driven settlement flow and establish the economic primitives that later automation in Phase 2 will depend on.

## What Changes

- Replace epoch-based settlement assumptions with receipt-based economic settlement controlled by the attestation program.
- Introduce a singleton global config that stores vault bindings, admin-controlled economic parameters, and the configured attestation program.
- Add provider registration and exit flows backed by a required USDC stake and exit-blocking obligation checks.
- Add per-user, per-provider, per-receipt, and per-challenge accounting accounts to snapshot settlement outcomes and enforce idempotent state transitions.
- Add attestation-only CPI entrypoints to record mining from receipts, record challenge bonds, settle finalized receipts, and resolve challenge economics.
- Add locked reward accounting for users and providers, including provider net-negative `CLAW` debt semantics and a claim path for already released rewards.
- Preserve the Phase 1 boundary by defining lock and release accounting rules without implementing the daily unlock executor.

## Capabilities

### New Capabilities
- `masterpool-phase1-config`: Initialize the Phase 1 masterpool, bind economic vaults, mint the fixed `CLAW` genesis supply, and manage admin-controlled parameter updates with invariant checks.
- `provider-staking-lifecycle`: Register providers with a USDC stake, track provider obligation state, and allow exit only when all blocking conditions are cleared.
- `receipt-settlement`: Record receipt-based USDC and `CLAW` economics from attestation CPI, escrow provider revenue, snapshot per-receipt economics, and release provider USDC only after finalized recognition.
- `challenge-economics`: Lock challenger bonds, resolve accepted or rejected challenges, refund the receipt payer when required, and apply provider penalty and burn/reward splits from snapshotted parameters.
- `locked-reward-accounting`: Track locked, released, and claimed `CLAW` for users and providers, apply provider rewards against negative net positions first, and expose a claim path for already released rewards.

### Modified Capabilities

None.

## Impact

- Affected program: `programs/clawfarm-masterpool`
- Likely affected modules: `programs/clawfarm-masterpool/src/lib.rs`, `programs/clawfarm-masterpool/src/state/accounts.rs`, `programs/clawfarm-masterpool/src/state/mod.rs`, `programs/clawfarm-masterpool/src/instructions/setup.rs`, `programs/clawfarm-masterpool/src/instructions/admin.rs`, `programs/clawfarm-masterpool/src/instructions/distribution.rs`, and new instruction/state submodules for provider, receipt, reward, and challenge flows
- Affected integration surface: attestation-to-masterpool CPI contract and Anchor tests covering receipt recording, finalization, challenge resolution, and claims
- Existing epoch-settlement flows will likely be removed or deprecated as part of the Phase 1 economic-model transition
