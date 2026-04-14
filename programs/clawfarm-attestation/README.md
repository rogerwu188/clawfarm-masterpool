# clawfarm-attestation

`clawfarm-attestation` is the dedicated Solana lifecycle program for Clawfarm
Phase 1 receipt attestation.

Chinese version:

- [README.zh-CN.md](README.zh-CN.md)

This README documents the current on-chain implementation in this repository.

Source of truth:

- [src/lib.rs](src/lib.rs)
- [src/instructions/admin.rs](src/instructions/admin.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [src/events.rs](src/events.rs)
- [../clawfarm-masterpool/README.md](../clawfarm-masterpool/README.md)
- [../../tests/phase1-integration.ts](../../tests/phase1-integration.ts)

## Responsibilities

- maintain the provider signer registry
- verify provider signatures over canonical receipt digests
- prevent replay by `request_nonce`
- manage receipt and challenge lifecycle state
- forward economic actions to `clawfarm-masterpool` through CPI
- close terminal `Receipt` and `Challenge` accounts to reclaim rent

## High-Level Model

Phase 1 uses a minimal on-chain receipt anchor.

The full receipt body stays off-chain. The program only stores:

- `receipt_hash`
- `signer`
- lifecycle timestamps
- receipt status
- whether economics have already been forwarded to masterpool

The trust boundary is:

1. the full receipt body is canonicalized off-chain
2. the program rebuilds the same canonical CBOR payload on-chain
3. the program checks `sha256(canonical_payload) == receipt_hash`
4. the program checks the preceding `ed25519` verification instruction
5. the program creates a minimal `Receipt` PDA keyed by `request_nonce`
6. the program CPIs into masterpool to record or settle the economic side

## Current Implementation Constraints

- only `version == 1` receipts are accepted
- only `ProofMode::SigLog` is accepted in Phase 1
- only `UsageBasis::ProviderReported` is accepted in Phase 1
- one `Receipt` PDA exists per `request_nonce`
- one `Challenge` PDA exists per `Receipt`
- there is no `respond_challenge` instruction
- full receipt bodies and proof URLs are not stored on-chain
- challenge bonds and challenge settlement live in masterpool as `CLAW`, not in
  this program as lamports
- `close_receipt` requires both a terminal receipt state and
  `receipt.economics_settled == true`
- if a challenge is rejected, the receipt becomes `Finalized` but still needs a
  later `finalize_receipt` call to settle provider payout in masterpool
- `initialize_config` is currently just a payer-funded singleton init; unlike
  older drafts, it does not check upgrade-authority `ProgramData`

## Signer Roles

- `authority`
  - submits receipts
  - finalizes uncontested receipts
  - finalizes challenge-rejected receipts after economics are still pending
  - closes terminal receipt and challenge accounts
- `pause_authority`
  - toggles the program pause flag
- `challenge_resolver`
  - resolves open challenges
- `challenger`
  - opens a challenge and pays the fixed `CLAW` bond through masterpool

## Program State

State definitions live in [src/state/accounts.rs](src/state/accounts.rs).

### `Config`

PDA seed:

- `["config"]`

Fields:

- `authority`
- `pause_authority`
- `challenge_resolver`
- `masterpool_program`
- `challenge_window_seconds`
- `is_paused`

Purpose:

- singleton governance config for lifecycle authorities, the linked
  `clawfarm-masterpool` program, and the challenge window

### `ProviderSigner`

PDA seed:

- `["provider_signer", sha256(provider_code), signer_pubkey]`

Fields:

- `attester_type_mask`
- `status`
- `valid_from`
- `valid_until`

Purpose:

- minimal signer policy keyed by provider code and signer pubkey

### `Receipt`

PDA seed:

- `["receipt", sha256(request_nonce)]`

Fields:

- `receipt_hash`
- `signer`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `status`
- `economics_settled`

Purpose:

- replay lock keyed by `request_nonce`
- minimal anchor for an off-chain receipt body
- state machine for submit, challenge, finalize, and close

### `Challenge`

PDA seed:

- `["challenge", receipt.key()]`

Fields:

- `receipt`
- `challenger`
- `challenge_type`
- `evidence_hash`
- `bond_amount`
- `opened_at`
- `resolved_at`
- `status`
- `resolution_code`

Purpose:

- one dispute slot for one receipt

## Enum Values

Definitions live in [src/state/types.rs](src/state/types.rs).

### `ProofMode`

- `0 = SigLog`
- `1 = SigLogZkReserved`

### `AttesterType`

- `0 = Provider`
- `1 = Gateway`
- `2 = Hybrid`

### `UsageBasis`

- `0 = ProviderReported`
- `1 = ServerEstimatedReserved`
- `2 = HybridReserved`
- `3 = TokenizerVerifiedReserved`

### `SignerStatus`

- `0 = Inactive`
- `1 = Active`
- `2 = Revoked`

### `ReceiptStatus`

- `0 = Submitted`
- `1 = Challenged`
- `2 = Finalized`
- `3 = Rejected`
- `4 = Slashed`

### `ChallengeStatus`

- `0 = Open`
- `1 = Accepted`
- `2 = Rejected`

### `ResolutionCode`

- `0 = None`
- `1 = Accepted`
- `2 = Rejected`
- `3 = ReceiptInvalidated`
- `4 = SignerRevoked`

## `SubmitReceiptArgs` Contract

`submit_receipt` receives the structured `SubmitReceiptArgs` payload.

Required fields:

- `version`
- `proof_mode`
- `proof_id`
- `request_nonce`
- `provider`
- `attester_type`
- `model`
- `usage_basis`
- `prompt_tokens`
- `completion_tokens`
- `total_tokens`
- `charge_atomic`
- `charge_mint`
- `receipt_hash`
- `signer`

Optional fields:

- `provider_request_id`
- `issued_at`
- `expires_at`
- `http_status`
- `latency_ms`

Validation rules currently enforced on-chain:

- `version` must be `1`
- `proof_mode` must be `SigLog`
- `usage_basis` must be `ProviderReported`
- `total_tokens` must equal `prompt_tokens + completion_tokens`
- string fields must respect Phase 1 length and character limits
- `http_status`, `issued_at`, and `expires_at` must be internally consistent
- the preceding transaction instruction must be a matching `ed25519` verify
  over the raw 32-byte digest

## Instruction Surface

Entry points live in [src/lib.rs](src/lib.rs).

### Admin

- `initialize_config(authority, pause_authority, challenge_resolver, masterpool_program, challenge_window_seconds)`
  - creates the singleton config
  - binds governance roles and the linked masterpool program
- `upsert_provider_signer(provider_code, signer, attester_type_mask, valid_from, valid_until)`
  - creates or updates the provider signer policy record
- `set_pause(is_paused)`
  - toggles the global pause flag
- `revoke_provider_signer(provider_code, signer)`
  - sets the signer status to revoked

### Receipt Lifecycle

- `submit_receipt(args: SubmitReceiptArgs)`
  - validates the structured payload
  - validates provider signer policy and time window
  - rebuilds canonical CBOR and verifies `receipt_hash`
  - verifies the preceding `ed25519` instruction
  - creates the `Receipt` PDA
  - CPIs into masterpool `record_mining_from_receipt`
- `finalize_receipt()`
  - finalizes a `Submitted` receipt after the challenge window closes
  - or settles a previously `Finalized` but not yet economically-settled
    receipt
  - CPIs into masterpool `settle_finalized_receipt`
  - sets `receipt.economics_settled = true`
- `close_receipt()`
  - closes a receipt only when the status is terminal and economics were already
    forwarded to masterpool

### Challenge Lifecycle

- `open_challenge(challenge_type, evidence_hash)`
  - validates the challenge type
  - requires a still-challengeable receipt
  - creates the `Challenge` PDA
  - CPIs into masterpool `record_challenge_bond`
  - moves the receipt into `Challenged`
- `resolve_challenge(resolution_code)`
  - requires an open challenge
  - sets terminal receipt and challenge state
  - CPIs into masterpool `resolve_challenge_economics`
  - keeps `economics_settled = false` only for the `Rejected` path, because
    provider payout still needs later finalization
- `close_challenge()`
  - closes a challenge after it reaches `Accepted` or `Rejected`

## Event Surface

Events are defined in [src/events.rs](src/events.rs).

- `ConfigInitialized`
- `ProviderSignerUpserted`
- `ProviderSignerRevoked`
- `PauseUpdated`
- `ReceiptSubmitted`
- `ReceiptFinalized`
- `ReceiptClosed`
- `ChallengeOpened`
- `ChallengeResolved`
- `ChallengeClosed`

## Lifecycle Flow

1. Admin initializes config and upserts one or more provider signer records.
2. `authority` submits a receipt with a matching preceding `ed25519`
   verification instruction.
3. Attestation creates the `Receipt` PDA and CPIs into masterpool to record the
   receipt economics.
4. A challenger may open exactly one challenge during the challenge window.
5. `challenge_resolver` resolves the challenge:
   - `Rejected`: receipt becomes `Finalized`, challenger bond is burned in
     masterpool, and economics still need later finalization
   - `Accepted` or `ReceiptInvalidated`: receipt becomes `Rejected` and
     economics are reverted in masterpool
   - `SignerRevoked`: receipt becomes `Slashed` and economics are reverted in
     masterpool
6. `authority` finalizes uncontested receipts, or challenge-rejected receipts
   that still need provider payout settlement.
7. After terminal state and economic settlement, `authority` closes the
   challenge and receipt accounts to reclaim rent.

## Tested Behavior

The current end-to-end integration test in
[../../tests/phase1-integration.ts](../../tests/phase1-integration.ts) covers:

- provider signer bootstrap
- receipt submission plus masterpool CPI recording
- unauthorized direct masterpool receipt recording failure
- rejected challenge plus later finalization
- accepted challenge refund and slash path
- duplicate receipt prevention
- close guards that enforce `economics_settled`

## Development

```bash
anchor build
anchor test
```
