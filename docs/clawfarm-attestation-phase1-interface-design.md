# Clawfarm Attestation Phase 1 Contract Interface Design

Status: Draft
Version: v0
Last Updated: 2026-04-03

## Scope

This document drafts the Phase 1 on-chain interface for the Clawfarm attestation flow, based on the AIRouter protocol and storage documents:

- `/Users/lijing/Code/Cobra/Adion/AIRouter/docs/clawfarm-attestation-protocol.md`
- `/Users/lijing/Code/Cobra/Adion/AIRouter/docs/clawfarm-attestation-cbor.md`
- `/Users/lijing/Code/Cobra/Adion/AIRouter/docs/clawfarm-solana-service-api.md`
- `/Users/lijing/Code/Cobra/Adion/AIRouter/docs/clawfarm-attestation-storage-design.md`

The design target is a dedicated Solana program for attestation receipts. It is intentionally separate from the current master-pool emission logic.

Phase 1 only supports `proof_mode = sig_log`.

## Goals

Phase 1 on-chain must do four things well:

1. prevent replay by `request_nonce`
2. verify the provider signer against a registry
3. verify the provider signature over the attestation payload
4. store a minimal receipt and challenge lifecycle

Phase 1 does not attempt to prove token correctness on-chain and does not own fee routing or treasury movement.

## Design Decisions

### 1. Separate Attestation Program

Use a dedicated program, tentatively named `clawfarm_attestation`, rather than merging this logic into `clawfarm-masterpool`.

Reason:

- signer registry, receipt replay protection, and challenge lifecycle are orthogonal to emission-vault logic
- ABI and account growth for attestation will be much faster than master-pool logic
- Phase 2 ZK extension should not force unrelated state migration in the vault program

### 2. Structured Receipt Args + Canonical Hash Rebuild

`submit_receipt` should receive structured receipt fields, not just a raw opaque blob.

The program should:

- rebuild the canonical Phase 1 payload from instruction args
- deterministically encode it using the same CBOR rules as AIRouter
- compute `sha256(canonical_cbor_bytes)`
- require that the digest matches `receipt_hash`
- require an `ed25519` verify instruction over that digest

Reason:

- replay protection only works if `request_nonce` is cryptographically bound to the signed payload
- passing only `receipt_hash` would force the program to trust the off-chain service for payload binding
- AIRouter Phase 1 payload is scalar-only and still small enough to justify an in-program canonical encoder

### 3. Minimal On-Chain Storage

Store the minimal fields needed for replay protection, audit correlation, and challenge handling:

- `request_nonce`
- `proof_id`
- `provider`
- `model`
- `proof_mode`
- `attester_type`
- `usage_basis`
- token counts
- `charge_atomic`
- `charge_mint`
- `receipt_hash`
- `signer`
- `proof_url_hash`
- lifecycle timestamps and status

Do not store the full proof bundle or transparency-log inclusion proof on-chain in Phase 1.

### 4. Governance-Driven Challenge Resolution

Phase 1 challenge resolution is authority-driven, not fully trustless.

Reason:

- AIRouter protocol already recommends governance-driven adjudication in v1
- log inclusion disputes and off-chain bundle parsing are better handled off-chain first

## Program State

## PDA: `Config`

Seed:

- `["config"]`

Fields:

- `authority: Pubkey`
- `pause_authority: Pubkey`
- `challenge_resolver: Pubkey`
- `challenge_window_seconds: i64`
- `response_window_seconds: i64`
- `receipt_count: u64`
- `challenge_count: u64`
- `is_paused: bool`
- `phase2_enabled: bool`
- `bump: u8`

Purpose:

- global governance and lifecycle settings

## PDA: `ProviderSigner`

Seed:

- `["provider_signer", provider_code_hash[32], signer_pubkey]`

Fields:

- `provider_code: String`
- `signer: Pubkey`
- `key_id: String`
- `attester_type_mask: u8`
- `status: u8`
- `valid_from: i64`
- `valid_until: i64`
- `metadata_hash: [u8; 32]`
- `created_at: i64`
- `updated_at: i64`
- `bump: u8`

Status:

- `0 = inactive`
- `1 = active`
- `2 = revoked`

Purpose:

- signer registry keyed by provider + signer

## PDA: `Receipt`

Seed:

- `["receipt", request_nonce_hash[32]]`

Fields:

- `request_nonce: String`
- `proof_id: String`
- `provider: String`
- `model: String`
- `proof_mode: u8`
- `attester_type: u8`
- `usage_basis: u8`
- `prompt_tokens: u64`
- `completion_tokens: u64`
- `total_tokens: u64`
- `charge_atomic: u64`
- `charge_mint: Pubkey`
- `receipt_hash: [u8; 32]`
- `signer: Pubkey`
- `proof_url_hash: [u8; 32]`
- `submitted_at: i64`
- `challenge_deadline: i64`
- `finalized_at: i64`
- `status: u8`
- `bump: u8`

Status:

- `0 = submitted`
- `1 = challenged`
- `2 = finalized`
- `3 = rejected`
- `4 = slashed`

Notes:

- `request_nonce` should still be stored in cleartext for auditability and service correlation
- `proof_url_hash = sha256(proof_url_utf8_bytes)`
- `charge_atomic` is parsed on submit from the decimal string in AIRouter and normalized to `u64`

## PDA: `Challenge`

Seed:

- `["challenge", receipt.key(), challenge_type_u8, challenger.key()]`

Fields:

- `request_nonce: String`
- `receipt: Pubkey`
- `challenger: Pubkey`
- `challenge_type: u8`
- `evidence_hash: [u8; 32]`
- `response_hash: [u8; 32]`
- `opened_at: i64`
- `response_deadline: i64`
- `resolved_at: i64`
- `status: u8`
- `resolution_code: u8`
- `bump: u8`

Status:

- `0 = open`
- `1 = responded`
- `2 = accepted`
- `3 = rejected`
- `4 = expired`

## Enum Mapping

### `proof_mode`

- `0 = sig_log`

Phase 1 rejects every other mode.

### `attester_type`

- `0 = provider`
- `1 = gateway`
- `2 = hybrid`

### `usage_basis`

- `0 = provider_reported`
- `1 = server_estimated` reserved for later
- `2 = hybrid` reserved for later
- `3 = tokenizer_verified` reserved for later

Phase 1 runtime should only accept `provider_reported`.

Reason:

- AIRouter currently has a schema drift between the protocol/schema docs and the Go shared types
- current Unipass settlement flow is naturally `provider_reported`
- freezing one accepted value keeps the first implementation auditable while leaving enum space open

### `challenge_type`

Initial Phase 1 values:

- `0 = invalid_signature`
- `1 = signer_registry_mismatch`
- `2 = replay_nonce`
- `3 = invalid_log_inclusion`
- `4 = payload_mismatch`

### `resolution_code`

- `0 = none`
- `1 = accepted`
- `2 = rejected`
- `3 = receipt_invalidated`
- `4 = signer_revoked`

## Phase 1 Instruction Set

## 1. `initialize_config`

Purpose:

- create the global config

Suggested signature:

```rust
pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    challenge_window_seconds: i64,
    response_window_seconds: i64,
) -> Result<()>
```

Checks:

- config does not already exist
- all durations are positive

## 2. `upsert_provider_signer`

Purpose:

- add or update a provider signer registry entry

Suggested signature:

```rust
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_code: String,
    signer: Pubkey,
    key_id: String,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
    metadata_hash: [u8; 32],
) -> Result<()>
```

Checks:

- only config authority
- `provider_code` non-empty
- `valid_until == 0 || valid_until >= valid_from`
- `attester_type_mask != 0`

Effects:

- creates or updates `ProviderSigner`
- marks status active

## 3. `set_pause`

Purpose:

- pause or unpause receipt submission and challenge mutations

Suggested signature:

```rust
pub fn set_pause(
    ctx: Context<SetPause>,
    is_paused: bool,
) -> Result<()>
```

Checks:

- only `pause_authority`

Effects:

- updates `config.is_paused`

## 4. `revoke_provider_signer`

Purpose:

- deactivate a signer without deleting history

Suggested signature:

```rust
pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    provider_code: String,
    signer: Pubkey,
) -> Result<()>
```

Checks:

- only config authority

Effects:

- sets signer status to revoked

## 5. `submit_receipt`

Purpose:

- verify and record a provider attestation receipt

Suggested args:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SubmitReceiptArgs {
    pub version: u8,
    pub proof_mode: u8,
    pub proof_id: String,
    pub request_nonce: String,
    pub provider: String,
    pub attester_type: u8,
    pub model: String,
    pub usage_basis: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub charge_atomic: u64,
    pub charge_mint: Pubkey,
    pub provider_request_id: Option<String>,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub proof_url: String,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
    pub signature: [u8; 64],
}
```

Suggested signature:

```rust
pub fn submit_receipt(
    ctx: Context<SubmitReceipt>,
    args: SubmitReceiptArgs,
) -> Result<()>
```

Checks:

- config not paused
- `version == 1`
- `proof_mode == sig_log`
- token arithmetic is valid
- `request_nonce` format is valid
- receipt PDA does not already exist
- signer registry entry exists and is active
- signer registry provider matches `provider`
- signer is valid for `attester_type`
- `issued_at` and `expires_at` are coherent
- deterministic CBOR rebuild hashes to `receipt_hash`
- the immediately preceding `ed25519` instruction verifies `signature` by `signer` over raw `receipt_hash`

Effects:

- creates `Receipt`
- sets status `submitted`
- sets `challenge_deadline = now + challenge_window_seconds`
- increments `receipt_count`

Implementation note:

- on-chain verification should use instruction introspection against the `ed25519_program`
- `signature` should still be stored in the instruction args for the introspection check, but does not need to remain in the account after successful submission

## 6. `open_challenge`

Purpose:

- open a dispute for an existing receipt

Suggested signature:

```rust
pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    request_nonce: String,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()>
```

Checks:

- receipt exists
- receipt status is `submitted` or `challenged`
- challenge window still open
- duplicate open challenge by same challenger and same type does not already exist

Effects:

- creates `Challenge`
- marks receipt `challenged`
- increments `challenge_count`

Phase 1 implementation note:

- only one active challenge is allowed per receipt at a time
- a new challenge can only be opened while receipt status is `submitted`

## 7. `respond_challenge`

Purpose:

- attach provider or resolver response evidence

Suggested signature:

```rust
pub fn respond_challenge(
    ctx: Context<RespondChallenge>,
    request_nonce: String,
    challenge_type: u8,
    response_hash: [u8; 32],
) -> Result<()>
```

Checks:

- challenge exists and is open
- caller is governance-authorized in Phase 1 (`authority` or `challenge_resolver`)

Effects:

- writes `response_hash`
- marks challenge `responded`

## 8. `resolve_challenge`

Purpose:

- finalize challenge outcome

Suggested signature:

```rust
pub fn resolve_challenge(
    ctx: Context<ResolveChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    resolution_code: u8,
) -> Result<()>
```

Checks:

- only `challenge_resolver`
- challenge exists and is open or responded
- resolution code is valid

Effects:

- marks challenge accepted or rejected
- updates receipt status
- if accepted, receipt becomes `rejected` or `slashed`
- if rejected and no remaining open challenge, receipt returns to `submitted`

## 9. `finalize_receipt`

Purpose:

- move an uncontested receipt to final state

Suggested signature:

```rust
pub fn finalize_receipt(
    ctx: Context<FinalizeReceipt>,
    request_nonce: String,
) -> Result<()>
```

Checks:

- receipt exists
- receipt status is `submitted`
- challenge window elapsed
- no open challenge exists
- no accepted challenge exists

Effects:

- marks receipt `finalized`
- records `finalized_at`

## Validation Path

The intended submit path is:

1. AIRouter validates the capsule shape off-chain.
2. The external Solana service rebuilds canonical CBOR and `receipt_hash`.
3. The service creates an `ed25519` verify instruction for `signer` and `receipt_hash`.
4. The service invokes `submit_receipt`.
5. The program:
   - rebuilds canonical CBOR again
   - recomputes `receipt_hash`
   - introspects the `ed25519` instruction
   - checks signer registry
   - checks replay by `request_nonce`
   - stores the receipt

This keeps the program authoritative for replay and signature validity while keeping proof bundle retrieval fully off-chain.

## Canonical Payload Drift Note

There is one important upstream document drift to freeze explicitly for Phase 1 implementation:

- AIRouter response schema and envelope examples include `proof_url`
- AIRouter `AttestationPayload` type and canonical CBOR documents do not include `proof_url` in the signed payload

Phase 1 implementation should follow the current AIRouter payload type and CBOR contract:

- `proof_url` is validated as an instruction field
- only `proof_url_hash = sha256(proof_url_utf8)` is stored on-chain
- `proof_url` is not included in the canonical CBOR payload used to recompute `receipt_hash`

If AIRouter later decides to sign `proof_url`, that should be treated as a payload-version change rather than a silent Phase 1 behavior change.

## Account Size Guidance

To keep allocation tractable in v1, impose explicit string caps:

- `request_nonce <= 128`
- `proof_id <= 128`
- `provider <= 64`
- `model <= 255`
- `provider_request_id <= 255`
- `proof_url <= 512`
- `key_id <= 128`

If a field exceeds the cap, reject submission.

## Events

Recommended events:

- `ConfigInitialized`
- `ProviderSignerUpserted`
- `ProviderSignerRevoked`
- `ReceiptSubmitted`
- `ReceiptFinalized`
- `ChallengeOpened`
- `ChallengeResponded`
- `ChallengeResolved`

Each receipt-related event should include:

- `request_nonce`
- `proof_id`
- `provider`
- `receipt_hash`
- `signer`

## Error Draft

Suggested error names:

- `ErrPaused`
- `ErrInvalidVersion`
- `ErrInvalidProofMode`
- `ErrInvalidNonce`
- `ErrInvalidProvider`
- `ErrInvalidAttesterType`
- `ErrInvalidUsageBasis`
- `ErrInvalidTokenTotals`
- `ErrReceiptExpired`
- `ErrReplayNonce`
- `ErrSignerNotFound`
- `ErrSignerInactive`
- `ErrSignerProviderMismatch`
- `ErrSignerAttesterTypeMismatch`
- `ErrReceiptHashMismatch`
- `ErrEd25519InstructionMissing`
- `ErrEd25519VerificationMismatch`
- `ErrReceiptNotChallengeable`
- `ErrChallengeAlreadyExists`
- `ErrChallengeWindowClosed`
- `ErrUnauthorizedResponder`
- `ErrUnauthorizedResolver`
- `ErrChallengeResolutionInvalid`

## What Phase 1 Explicitly Does Not Do

- no on-chain proof bundle fetch
- no on-chain transparency-log verification
- no ZK verification
- no treasury transfer or fee split
- no signer slashing economics
- no direct lookup PDA by `proof_id`

Those can be added in Phase 2 or in the off-chain service layer.

## Recommended Next Step

Before implementation, the next artifact should be a machine-readable ABI draft:

- `instructions.md` or `idl-sketch.json`
- enum discriminants frozen
- exact account space constants frozen
- exact canonical CBOR field order mirrored in test vectors

Once that is written, implementation can start with:

1. config and signer registry
2. submit path with `ed25519` introspection
3. receipt finalization
4. challenge lifecycle
