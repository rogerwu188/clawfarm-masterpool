# Clawfarm Attestation Phase 1 Contract Interface Design

Status: Implemented
Version: v1
Last Updated: 2026-04-03

## Scope

This document describes the current Phase 1 on-chain interface of the dedicated
`clawfarm_attestation` Solana program in this repository.

Phase 1 is intentionally narrow:

- verify a provider or gateway signer against an on-chain registry
- verify an `ed25519` signature over a canonical receipt digest
- prevent replay by `request_nonce`
- keep only a minimal on-chain receipt anchor
- allow a governance-driven challenge lifecycle
- close terminal receipt and challenge accounts to reclaim rent

Phase 1 does not:

- store the full receipt body on-chain
- fetch data from S3, IPFS, or any off-chain endpoint
- route funds or settle treasury balances
- do trustless proof verification beyond signer and digest validation

## Design Summary

### 1. Dedicated Attestation Program

Attestation state is kept separate from `clawfarm-masterpool`.

Reason:

- signer registry and dispute state have a different growth profile
- receipt storage and challenge flow should not bloat the pool program
- future proof modes can evolve without migrating vault logic

### 2. Structured Receipt Args + Canonical Hash Rebuild

`submit_receipt` accepts a structured `SubmitReceiptArgs`, rebuilds the Phase 1
canonical CBOR payload on-chain, hashes it with `sha256`, and requires the
result to match `receipt_hash`.

Reason:

- `request_nonce` replay protection must be cryptographically bound to the
  signed payload
- the program must not trust an off-chain service to compute the only binding
  hash

### 3. ReceiptLite On-Chain Storage

Phase 1 stores only the minimal fields needed to anchor and adjudicate a
receipt:

- `receipt_hash`
- `signer`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `status`
- `bump`

The full receipt body is expected to live off-chain, for example in Clawfarm
managed S3 storage.

Reason:

- on-chain rent dominates cost; large strings are not worth storing twice
- the digest is enough to bind off-chain content to an on-chain state machine

### 4. Single-Path Terminal Resolution

Once a receipt leaves the active dispute state, it goes directly to a terminal
state:

- unchallenged and window elapsed: `Finalized`
- challenge rejected: `Finalized`
- challenge accepted: `Rejected` or `Slashed`

It does not return to `Submitted`.

Reason:

- the protocol only needs one adjudicated outcome per receipt
- this lets the account be closed immediately after the terminal result is
  observed

### 5. Explicit Rent Reclaim

Phase 1 includes `close_challenge` and `close_receipt`.

Reason:

- the transaction fee on Solana is small
- the real cost is account rent held by `Receipt` and `Challenge`
- reclaiming rent after terminal state is the main cost optimization

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
- `reserved: [u8; 32]`

Purpose:

- global governance and timing configuration

Notes:

- current runtime only checks `is_paused` inside `submit_receipt`

## PDA: `ProviderSigner`

Seed:

- `["provider_signer", sha256(provider_code), signer_pubkey]`

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
- `reserved: [u8; 32]`

Purpose:

- signer registry keyed by provider code and signer public key

## PDA: `Receipt`

Seed:

- `["receipt", sha256(request_nonce)]`

Fields:

- `receipt_hash: [u8; 32]`
- `signer: Pubkey`
- `submitted_at: i64`
- `challenge_deadline: i64`
- `finalized_at: i64`
- `status: u8`
- `bump: u8`

Purpose:

- replay lock keyed by `request_nonce`
- on-chain anchor for an off-chain canonical receipt
- lifecycle state for challenge and close

Notes:

- `request_nonce` is not stored in the account; it survives only in PDA derivation
- `proof_id`, `provider`, `model`, token counts, and `proof_url` stay off-chain
- `ReceiptSubmitted` event still exposes `request_nonce`, `proof_id`, and `provider`
  for indexing convenience

## PDA: `Challenge`

Seed:

- `["challenge", receipt.key(), challenge_type_u8, challenger.key()]`

Fields:

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

Purpose:

- store one challenger-specific dispute against one receipt and one challenge type

Notes:

- `request_nonce` is not stored; it is only used to derive the receipt PDA

## Enum Mapping

### `ProofMode`

- `0 = SigLog`
- `1 = SigLogZkReserved`

Phase 1 accepts only `0`.

### `AttesterType`

- `0 = Provider`
- `1 = Gateway`
- `2 = Hybrid`

### `UsageBasis`

- `0 = ProviderReported`
- `1 = ServerEstimatedReserved`
- `2 = HybridReserved`
- `3 = TokenizerVerifiedReserved`

Phase 1 accepts only `0`.

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
- `1 = Responded`
- `2 = Accepted`
- `3 = Rejected`
- `4 = Expired`

Note:

- `Expired` exists in the enum space but Phase 1 does not currently implement an
  instruction that transitions into it

### `ChallengeType`

- `0 = InvalidSignature`
- `1 = SignerRegistryMismatch`
- `2 = ReplayNonce`
- `3 = InvalidLogInclusion`
- `4 = PayloadMismatch`

### `ResolutionCode`

- `0 = None`
- `1 = Accepted`
- `2 = Rejected`
- `3 = ReceiptInvalidated`
- `4 = SignerRevoked`

## Instruction Set

## 1. `initialize_config`

Purpose:

- create the global config PDA

Signature:

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

- `challenge_window_seconds > 0`
- `response_window_seconds > 0`

Effects:

- creates `Config`
- initializes counters to zero
- sets `is_paused = false`
- sets `phase2_enabled = false`

## 2. `upsert_provider_signer`

Purpose:

- create or update a provider signer registry entry

Signature:

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

- only `config.authority`
- valid `provider_code`
- valid `key_id`
- `attester_type_mask != 0`
- `valid_until == 0 || valid_until >= valid_from`

Effects:

- creates or updates `ProviderSigner`
- forces status to `Active`
- preserves `created_at` on updates

## 3. `set_pause`

Purpose:

- update `config.is_paused`

Signature:

```rust
pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()>
```

Checks:

- only `config.pause_authority`

Effects:

- sets `config.is_paused`

## 4. `revoke_provider_signer`

Purpose:

- revoke a signer without deleting history

Signature:

```rust
pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    provider_code: String,
    signer: Pubkey,
) -> Result<()>
```

Checks:

- only `config.authority`
- signer PDA fields match provided `provider_code` and `signer`

Effects:

- sets signer status to `Revoked`

## 5. `submit_receipt`

Purpose:

- verify and record a provider attestation receipt

Args:

```rust
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

Checks:

- program not paused
- `version == 1`
- `proof_mode == SigLog`
- `usage_basis == ProviderReported`
- `total_tokens == prompt_tokens + completion_tokens`
- string caps and format validation succeed
- provider signer exists and is active
- signer registry matches `provider`, `signer`, and `attester_type`
- signer validity window includes `now`
- on-chain canonical CBOR rebuild hashes to `receipt_hash`
- preceding `ed25519` verify instruction matches `signer`, `signature`, and `receipt_hash`

Effects:

- creates `Receipt`
- stores only minimal anchor fields
- sets `status = Submitted`
- sets `challenge_deadline = now + challenge_window_seconds`
- increments `config.receipt_count`

## 6. `open_challenge`

Purpose:

- open a dispute for an active receipt

Signature:

```rust
pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    request_nonce: String,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()>
```

Checks:

- `request_nonce` format is valid
- `challenge_type` is valid
- receipt PDA exists
- receipt status is `Submitted`
- current time is not past `challenge_deadline`

Effects:

- creates `Challenge`
- sets `challenge.status = Open`
- sets `receipt.status = Challenged`
- increments `config.challenge_count`

## 7. `respond_challenge`

Purpose:

- attach resolver-side response evidence to an open challenge

Signature:

```rust
pub fn respond_challenge(
    ctx: Context<RespondChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    response_hash: [u8; 32],
) -> Result<()>
```

Checks:

- `request_nonce` format is valid
- `challenge_type` is valid
- caller is `config.authority` or `config.challenge_resolver`
- targeted challenge is still `Open`
- current time is not past `response_deadline`

Effects:

- sets `response_hash`
- sets `challenge.status = Responded`

## 8. `resolve_challenge`

Purpose:

- finalize a challenge and move the receipt directly into terminal state

Signature:

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

- only `config.challenge_resolver`
- `request_nonce` format is valid
- `challenge_type` is valid
- `resolution_code` is valid and not `None`
- challenge is `Open` or `Responded`

Effects:

- if `resolution_code` is `Accepted` or `ReceiptInvalidated`
  - `challenge.status = Accepted`
  - `receipt.status = Rejected`
  - `receipt.finalized_at = now`
- if `resolution_code` is `SignerRevoked`
  - `challenge.status = Accepted`
  - `receipt.status = Slashed`
  - `receipt.finalized_at = now`
- if `resolution_code` is `Rejected`
  - `challenge.status = Rejected`
  - `receipt.status = Finalized`
  - `receipt.finalized_at = now`

## 9. `finalize_receipt`

Purpose:

- finalize an uncontested receipt after the challenge window ends

Signature:

```rust
pub fn finalize_receipt(
    ctx: Context<FinalizeReceipt>,
    request_nonce: String,
) -> Result<()>
```

Checks:

- `request_nonce` format is valid
- receipt status is `Submitted`
- current time is greater than `challenge_deadline`

Effects:

- sets `receipt.status = Finalized`
- sets `receipt.finalized_at = now`

## 10. `close_challenge`

Purpose:

- reclaim rent from a terminal challenge account

Signature:

```rust
pub fn close_challenge(
    ctx: Context<CloseChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
) -> Result<()>
```

Checks:

- `request_nonce` format is valid
- `challenge_type` is valid
- challenge status is `Accepted`, `Rejected`, or `Expired`

Effects:

- closes `Challenge`
- transfers lamports to `recipient`

## 11. `close_receipt`

Purpose:

- reclaim rent from a terminal receipt account

Signature:

```rust
pub fn close_receipt(
    ctx: Context<CloseReceipt>,
    request_nonce: String,
) -> Result<()>
```

Checks:

- `request_nonce` format is valid
- receipt status is `Finalized`, `Rejected`, or `Slashed`

Effects:

- closes `Receipt`
- transfers lamports to `recipient`

## State Transitions

Receipt lifecycle:

```text
Submitted
  -> Challenged
  -> Finalized      (finalize_receipt after window)

Challenged
  -> Finalized      (challenge rejected)
  -> Rejected       (challenge accepted / receipt invalidated)
  -> Slashed        (challenge accepted / signer revoked)
```

Closable receipt states:

- `Finalized`
- `Rejected`
- `Slashed`

Challenge lifecycle:

```text
Open
  -> Responded
  -> Accepted
  -> Rejected

Responded
  -> Accepted
  -> Rejected
```

Closable challenge states:

- `Accepted`
- `Rejected`
- `Expired`

## Canonicalization Contract

The canonical receipt digest is:

- deterministic CBOR map encoding
- same logical Phase 1 payload fields as the current off-chain schema
- omitted optional fields are not encoded
- `receipt_hash = sha256(canonical_cbor_bytes)`
- `ed25519` signs the raw 32-byte digest

Fields excluded from the signed payload:

- `proof_url`
- `signer`
- `signature`
- `receipt_hash`

`proof_url` is validated as a transport field but not stored on-chain in Phase 1.

## Recommended Off-Chain Storage Flow

The current on-chain design is intended to pair with a Clawfarm managed off-chain
receipt service, for example backed by S3.

Recommended flow:

1. Provider returns the full receipt body to Clawfarm.
2. Clawfarm normalizes it into the canonical Phase 1 payload.
3. Clawfarm computes canonical CBOR and `receipt_hash`.
4. Clawfarm uploads the canonical receipt object to S3.
5. Clawfarm submits `submit_receipt` with the structured fields and matching
   signer verification.
6. Clawfarm website indexes the receipt by `receipt_hash`, `request_nonce`, and
   provider-side identifiers.
7. A challenger retrieves the full receipt from Clawfarm infrastructure, prepares
   evidence, and submits only evidence hashes on-chain.
8. After terminal state, Clawfarm closes the `Challenge` and `Receipt` accounts
   to reclaim rent.

Implications:

- S3 is an availability layer, not the trust anchor
- the trust anchor is still the on-chain `receipt_hash`
- receipt authenticity is derived from canonical hash + signer verification
- rent usage is bounded by the lifetime of the minimal on-chain accounts

## Account Size Guidance

Current allocation uses `8 + <Account>::INIT_SPACE`.

Effective sizes in the current implementation:

- `Config = 171 bytes`
- `ProviderSigner = 339 bytes`
- `Receipt = 98 bytes`
- `Challenge = 164 bytes`

This is the main rent reduction relative to the previous design that stored the
full receipt body on-chain.

## Events

Current events:

- `ConfigInitialized`
- `ProviderSignerUpserted`
- `ProviderSignerRevoked`
- `PauseUpdated`
- `ReceiptSubmitted`
- `ReceiptFinalized`
- `ReceiptClosed`
- `ChallengeOpened`
- `ChallengeResponded`
- `ChallengeResolved`
- `ChallengeClosed`
