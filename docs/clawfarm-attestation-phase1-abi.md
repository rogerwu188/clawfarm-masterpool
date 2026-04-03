# Clawfarm Attestation Phase 1 ABI

Status: Implemented
Version: v1
Last Updated: 2026-04-03

This document reflects the current ABI of the dedicated
`clawfarm_attestation` Solana program in this repository.

Reference:

- [clawfarm-attestation-phase1-interface-design.md](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/docs/clawfarm-attestation-phase1-interface-design.md)

## Phase 1 Freeze

The following are frozen in the current implementation:

- dedicated program name: `clawfarm_attestation`
- accepted `proof_mode`: `sig_log` only
- accepted `usage_basis`: `provider_reported` only
- signature algorithm: `ed25519`
- replay key: `request_nonce`
- on-chain receipt shape: `ReceiptLite`
- challenge resolution: governance-driven
- terminal account close flow: enabled

## Constants

## String Caps

```text
MAX_REQUEST_NONCE_LEN        = 128
MAX_PROOF_ID_LEN             = 128
MAX_PROVIDER_LEN             = 64
MAX_MODEL_LEN                = 255
MAX_PROVIDER_REQUEST_ID_LEN  = 255
```

## PDA Seeds

```text
CONFIG_SEED           = "config"
PROVIDER_SIGNER_SEED  = "provider_signer"
RECEIPT_SEED          = "receipt"
CHALLENGE_SEED        = "challenge"
```

## Enum Discriminants

### `ProofMode`

```text
0 = sig_log
1 = sig_log_zk_reserved
```

Phase 1 accepts only `0`.

### `AttesterType`

```text
0 = provider
1 = gateway
2 = hybrid
```

### `UsageBasis`

```text
0 = provider_reported
1 = server_estimated_reserved
2 = hybrid_reserved
3 = tokenizer_verified_reserved
```

Phase 1 accepts only `0`.

### `SignerStatus`

```text
0 = inactive
1 = active
2 = revoked
```

### `ReceiptStatus`

```text
0 = submitted
1 = challenged
2 = finalized
3 = rejected
4 = slashed
```

### `ChallengeStatus`

```text
0 = open
1 = accepted
2 = rejected
3 = expired
```

Note:

- `expired` is part of the enum but no current instruction writes it

### `ChallengeType`

```text
0 = invalid_signature
1 = signer_registry_mismatch
2 = replay_nonce
3 = invalid_log_inclusion
4 = payload_mismatch
```

### `ResolutionCode`

```text
0 = none
1 = accepted
2 = rejected
3 = receipt_invalidated
4 = signer_revoked
```

## Account Shapes

All runtime allocations use `8 + <Account>::INIT_SPACE`.

## `Config`

Logical fields:

```text
authority: Pubkey
pause_authority: Pubkey
challenge_resolver: Pubkey
challenge_window_seconds: i64
is_paused: bool
```

Account size:

```text
INIT_SPACE = 105
ALLOCATED  = 113
```

## `ProviderSigner`

Logical fields:

```text
attester_type_mask: u8
status: u8
valid_from: i64
valid_until: i64
```

Account size:

```text
INIT_SPACE = 18
ALLOCATED  = 26
```

## `Receipt`

Logical fields:

```text
receipt_hash: [u8; 32]
signer: Pubkey
submitted_at: i64
challenge_deadline: i64
finalized_at: i64
status: u8
```

Account size:

```text
INIT_SPACE = 89
ALLOCATED  = 97
```

Notes:

- this is the Phase 1 `ReceiptLite` shape
- `request_nonce`, `proof_id`, `provider`, and detailed usage metadata are not stored in the account

## `Challenge`

Logical fields:

```text
receipt: Pubkey
challenger: Pubkey
challenge_type: u8
evidence_hash: [u8; 32]
opened_at: i64
resolved_at: i64
status: u8
resolution_code: u8
```

Account size:

```text
INIT_SPACE = 115
ALLOCATED  = 123
```

## Instruction ABI

## 1. `initialize_config`

Args:

```rust
authority: Pubkey
pause_authority: Pubkey
challenge_resolver: Pubkey
challenge_window_seconds: i64
```

Accounts:

```text
[writable, signer] payer
[writable]         config
[]                 system_program
```

## 2. `upsert_provider_signer`

Args:

```rust
provider_code: String
signer: Pubkey
attester_type_mask: u8
valid_from: i64
valid_until: i64
```

Accounts:

```text
[writable, signer] authority
[]                 config
[writable]         provider_signer
[]                 system_program
```

Behavior:

- `init_if_needed`

## 3. `set_pause`

Args:

```rust
is_paused: bool
```

Accounts:

```text
[signer]           pause_authority
[writable]         config
```

## 4. `revoke_provider_signer`

Args:

```rust
provider_code: String
signer: Pubkey
```

Accounts:

```text
[signer]           authority
[]                 config
[writable]         provider_signer
```

## 5. `submit_receipt`

Args:

```rust
SubmitReceiptArgs {
  version: u8,
  proof_mode: u8,
  proof_id: String,
  request_nonce: String,
  provider: String,
  attester_type: u8,
  model: String,
  usage_basis: u8,
  prompt_tokens: u64,
  completion_tokens: u64,
  total_tokens: u64,
  charge_atomic: u64,
  charge_mint: Pubkey,
  provider_request_id: Option<String>,
  issued_at: Option<i64>,
  expires_at: Option<i64>,
  http_status: Option<u16>,
  latency_ms: Option<u64>,
  receipt_hash: [u8; 32],
  signer: Pubkey,
}
```

Accounts:

```text
[writable, signer] authority
[]                 config
[]                 provider_signer
[writable]         receipt
[]                 instructions_sysvar
[]                 system_program
```

Runtime expectation:

- the immediately preceding instruction must be the matching `ed25519_program`
  verify instruction for `signer` and `receipt_hash`

## 6. `open_challenge`

Args:

```rust
challenge_type: u8
evidence_hash: [u8; 32]
```

Accounts:

```text
[writable, signer] challenger
[writable]         receipt
[writable]         challenge
[]                 system_program
```

Runtime rule:

- receipt must be in `submitted` state

## 7. `resolve_challenge`

Args:

```rust
resolution_code: u8
```

Accounts:

```text
[signer]           challenge_resolver
[]                 config
[writable]         receipt
[writable]         challenge
```

Runtime rules:

- `config` must `has_one = challenge_resolver`
- `challenge.receipt` must equal `receipt.key()`

Terminal result mapping:

```text
accepted / receipt_invalidated -> receipt.rejected
signer_revoked                 -> receipt.slashed
rejected                       -> receipt.finalized
```

## 8. `finalize_receipt`

Args:

```rust
(none)
```

Accounts:

```text
[writable]         receipt
```

Runtime rule:

- receipt must still be `submitted` and the challenge window must be over

## 9. `close_challenge`

Args:

```rust
(none)
```

Accounts:

```text
[signer]           authority
[]                 config
[writable]         challenge
```

Runtime rule:

- `authority` must satisfy `config.has_one = authority`
- challenge must already be terminal

## 10. `close_receipt`

Args:

```rust
(none)
```

Accounts:

```text
[signer]           authority
[]                 config
[writable]         receipt
```

Runtime rule:

- receipt must already be terminal

## Canonicalization Contract

The current canonicalization contract is:

- on-chain rebuild from structured fields
- deterministic CBOR map encoding
- absent optional fields are omitted
- `receipt_hash = sha256(canonical_cbor_bytes)`
- `ed25519` signs the raw 32-byte digest

Fields intentionally excluded from the signed payload:

- `signer`
- `receipt_hash`

## Validation Rules

`submit_receipt` rejects when:

- `version != 1`
- `proof_mode != sig_log`
- `usage_basis != provider_reported`
- string caps are exceeded
- `request_nonce` format is invalid
- `total_tokens != prompt_tokens + completion_tokens`
- timestamps are inconsistent
- HTTP status is outside `100..=599`
- signer registry entry is missing or inactive
- signer registry provider mismatches
- signer is not authorized for the requested `attester_type`
- `request_nonce` PDA already exists
- canonical hash mismatches
- `ed25519` verify instruction is missing or mismatched

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
- `ChallengeResolved`
- `ChallengeClosed`
