## 1. Program skeleton and state model

- [x] 1.1 Replace the epoch-settlement-oriented `clawfarm-masterpool` entrypoint/module wiring with Phase 1 config, provider, receipt, reward, and challenge instruction modules
- [x] 1.2 Define the new Phase 1 account types and enums in `programs/clawfarm-masterpool/src/state` for global config, provider state, reward accounts, receipt settlements, and challenge-bond records
- [x] 1.3 Add PDA seeds, sizing constants, and shared validation helpers for the new state machine and vault layout

## 2. Config, vaults, and admin controls

- [x] 2.1 Implement masterpool initialization that binds admin, attestation program, mints, and all required vault accounts
- [x] 2.2 Implement the one-time fixed `CLAW` genesis mint flow into the reward vault and remove routine dependence on long-lived mint authority
- [x] 2.3 Implement config updates with split-sum invariants, positive amount checks, and future-only parameter semantics
- [x] 2.4 Add pause or guard controls needed to stop unsafe receipt, challenge, finalization, or claim flows during rollout

## 3. Provider lifecycle and reward accounting

- [x] 3.1 Implement provider registration with required USDC stake transfer and initialization of provider aggregate and reward accounts
- [x] 3.2 Implement provider exit checks for pending provider USDC, unsettled receipt count, unresolved challenge economics, and negative `CLAW` net position
- [x] 3.3 Implement aggregate user/provider reward-account bookkeeping for locked, released, and claimed `CLAW` totals
- [x] 3.4 Implement `claim_released_claw` for user and provider reward owners using released-minus-claimed accounting

## 4. Receipt-driven settlement flow

- [x] 4.1 Implement attestation-only `record_mining_from_receipt` with CPI caller verification, payer USDC charge, treasury split, provider escrow, and per-receipt snapshot creation
- [x] 4.2 Implement provider reward debt-offset logic so negative provider net positions are reduced before new locked rewards are created
- [x] 4.3 Implement attestation-only `settle_finalized_receipt` that releases escrowed provider USDC only for `Finalized` receipts still in `Recorded` settlement state
- [x] 4.4 Add idempotency and invalid-transition guards for duplicate receipt recording, duplicate payout, and payout attempts tied to rejected or slashed receipts

## 5. Challenge economics and attestation integration

- [x] 5.1 Implement attestation-only `record_challenge_bond` that escrows fixed `CLAW` collateral and snapshots slash/burn parameters per challenge
- [x] 5.2 Implement attestation-only `resolve_challenge_economics` for both rejected-challenge burn flow and accepted-challenge refund/slash flow
- [x] 5.3 Update `clawfarm-attestation` to invoke masterpool CPI hooks for receipt recording, challenge-bond recording, finalized settlement, and challenge-economics resolution
- [x] 5.4 Align attestation challenge collateral handling with the new masterpool-managed `CLAW` bond model and remove the old lamport-bond assumption

## 6. Verification and rollout readiness

- [x] 6.1 Rewrite or replace outdated masterpool tests to cover provider registration, receipt recording, finalized payout, successful challenge refund, rejected challenge burn, and reward claims
- [x] 6.2 Add cross-program integration tests that exercise the attestation-to-masterpool CPI path and verify state-machine invariants under duplicate or out-of-order calls
- [x] 6.3 Update repository docs to describe the Phase 1 economic model, the new account layout, and any intentionally removed epoch-settlement behavior
