# Clawfarm Attestation Phase 1 ABI Draft

Status: Draft
Version: v0
Last Updated: 2026-04-03

This document freezes the first implementation-oriented ABI for the dedicated `clawfarm_attestation` Solana program.

It is the next artifact after:

- [clawfarm-attestation-phase1-interface-design.md](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/docs/clawfarm-attestation-phase1-interface-design.md)

## Phase 1 Freeze

The following are frozen for the first implementation pass:

- dedicated program name: `clawfarm_attestation`
- accepted `proof_mode`: `sig_log` only
- accepted `usage_basis`: `provider_reported` only
- signature algorithm: `ed25519`
- replay key: `request_nonce`
- proof bundle handling: off-chain only
- challenge resolution: governance-driven

The following are intentionally not frozen yet:

- final program id
- event field ordering
- exact error code integers

## Constants

## String Caps

These caps are enforced at instruction validation time and baked into account space formulas.

```text
MAX_REQUEST_NONCE_LEN        = 128
MAX_PROOF_ID_LEN             = 128
MAX_PROVIDER_LEN             = 64
MAX_MODEL_LEN                = 255
MAX_KEY_ID_LEN               = 128
MAX_PROVIDER_REQUEST_ID_LEN  = 255
MAX_PROOF_URL_LEN            = 512
```

## PDA Seeds

```text
CONFIG_SEED           = "config"
PROVIDER_SIGNER_SEED  = "provider_signer"
RECEIPT_SEED          = "receipt"
CHALLENGE_SEED        = "challenge"
```

## Enum Discriminants

These discriminants should not change once implementation starts.

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
1 = responded
2 = accepted
3 = rejected
4 = expired
```

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

## `Config`

Logical fields:

```text
authority: Pubkey
pause_authority: Pubkey
challenge_resolver: Pubkey
challenge_window_seconds: i64
response_window_seconds: i64
receipt_count: u64
challenge_count: u64
is_paused: bool
phase2_enabled: bool
bump: u8
reserved: [u8; 32]
```

Suggested max space:

```text
CONFIG_SPACE = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 32 = 171
```

Round up to:

```text
CONFIG_SPACE = 192
```

## `ProviderSigner`

Logical fields:

```text
provider_code: String              // <= 64
signer: Pubkey
key_id: String                     // <= 128
attester_type_mask: u8
status: u8
valid_from: i64
valid_until: i64
metadata_hash: [u8; 32]
created_at: i64
updated_at: i64
bump: u8
reserved: [u8; 32]
```

Suggested max space:

```text
PROVIDER_SIGNER_SPACE =
  8
  + 4 + 64
  + 32
  + 4 + 128
  + 1
  + 1
  + 8
  + 8
  + 32
  + 8
  + 8
  + 1
  + 32
= 339
```

Round up to:

```text
PROVIDER_SIGNER_SPACE = 384
```

## `Receipt`

Logical fields:

```text
request_nonce: String              // <= 128
proof_id: String                   // <= 128
provider: String                   // <= 64
model: String                      // <= 255
proof_mode: u8
attester_type: u8
usage_basis: u8
prompt_tokens: u64
completion_tokens: u64
total_tokens: u64
charge_atomic: u64
charge_mint: Pubkey
receipt_hash: [u8; 32]
signer: Pubkey
proof_url_hash: [u8; 32]
submitted_at: i64
challenge_deadline: i64
finalized_at: i64
status: u8
bump: u8
reserved: [u8; 64]
```

Suggested max space:

```text
RECEIPT_SPACE =
  8
  + 4 + 128
  + 4 + 128
  + 4 + 64
  + 4 + 255
  + 1
  + 1
  + 1
  + 8
  + 8
  + 8
  + 8
  + 32
  + 32
  + 32
  + 8
  + 8
  + 8
  + 1
  + 1
  + 64
= 818
```

Round up to:

```text
RECEIPT_SPACE = 896
```

## `Challenge`

Logical fields:

```text
request_nonce: String              // <= 128
receipt: Pubkey
challenger: Pubkey
challenge_type: u8
evidence_hash: [u8; 32]
response_hash: [u8; 32]
opened_at: i64
response_deadline: i64
resolved_at: i64
status: u8
resolution_code: u8
bump: u8
reserved: [u8; 32]
```

Suggested max space:

```text
CHALLENGE_SPACE =
  8
  + 4 + 128
  + 32
  + 32
  + 1
  + 32
  + 32
  + 8
  + 8
  + 8
  + 1
  + 1
  + 1
  + 32
= 328
```

Round up to:

```text
CHALLENGE_SPACE = 352
```

## Instruction ABI

## 1. `initialize_config`

Args:

```rust
authority: Pubkey
pause_authority: Pubkey
challenge_resolver: Pubkey
challenge_window_seconds: i64
response_window_seconds: i64
```

Accounts:

```text
[writable, signer] payer
[writable]         config_pda
[]                 system_program
```

## 2. `upsert_provider_signer`

Args:

```rust
provider_code: String
signer: Pubkey
key_id: String
attester_type_mask: u8
valid_from: i64
valid_until: i64
metadata_hash: [u8; 32]
```

Accounts:

```text
[signer]           authority
[]                 config_pda
[writable]         provider_signer_pda
[]                 system_program
```

Behavior:

- init-if-needed semantics are acceptable

## 3. `set_pause`

Args:

```rust
is_paused: bool
```

Accounts:

```text
[signer]           pause_authority
[writable]         config_pda
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
[]                 config_pda
[writable]         provider_signer_pda
```

## 5. `submit_receipt`

Args:

```rust
version: u8
proof_mode: u8
proof_id: String
request_nonce: String
provider: String
attester_type: u8
model: String
usage_basis: u8
prompt_tokens: u64
completion_tokens: u64
total_tokens: u64
charge_atomic: u64
charge_mint: Pubkey
provider_request_id: Option<String>
issued_at: Option<i64>
expires_at: Option<i64>
http_status: Option<u16>
latency_ms: Option<u64>
proof_url: String
receipt_hash: [u8; 32]
signer: Pubkey
signature: [u8; 64]
```

Accounts:

```text
[writable, signer] payer
[]                 config_pda
[]                 provider_signer_pda
[writable]         receipt_pda
[]                 instructions_sysvar
[]                 system_program
```

Additional runtime expectation:

- the transaction must include the matching `ed25519_program` verify instruction immediately before this instruction

## 6. `open_challenge`

Args:

```rust
request_nonce: String
challenge_type: u8
evidence_hash: [u8; 32]
```

Accounts:

```text
[writable, signer] challenger
[]                 config_pda
[writable]         receipt_pda
[writable]         challenge_pda
[]                 system_program
```

## 7. `respond_challenge`

Args:

```rust
request_nonce: String
challenge_type: u8
challenger: Pubkey
response_hash: [u8; 32]
```

Accounts:

```text
[signer]           responder
[]                 config_pda
[]                 receipt_pda
[writable]         challenge_pda
```

## 8. `resolve_challenge`

Args:

```rust
request_nonce: String
challenge_type: u8
challenger: Pubkey
resolution_code: u8
```

Accounts:

```text
[signer]           challenge_resolver
[]                 config_pda
[writable]         receipt_pda
[writable]         challenge_pda
```

## 9. `finalize_receipt`

Args:

```rust
request_nonce: String
```

Accounts:

```text
[signer]           caller
[]                 config_pda
[writable]         receipt_pda
```

Phase 1 allows any caller because finalization is a deterministic state transition.

## Canonicalization Contract

The on-chain canonicalization contract should be:

- same logical fields as AIRouter canonical payload
- deterministic CBOR map encoding
- omitted optional fields must not appear
- `receipt_hash = sha256(canonical_cbor_bytes)`
- ed25519 signs raw 32-byte digest

Fields intentionally excluded from the signed payload in Phase 1:

- `signature`
- `signer`
- `proof_url_hash`

`proof_url` is included in the signed payload. The account stores only `proof_url_hash`.

## Validation Rules

`submit_receipt` must reject when:

- `version != 1`
- `proof_mode != sig_log`
- `usage_basis != provider_reported`
- string cap exceeded
- `total_tokens != prompt_tokens + completion_tokens`
- `expires_at < issued_at`
- signer registry entry missing or inactive
- signer registry provider mismatch
- signer not allowed for `attester_type`
- `request_nonce` already submitted
- canonical hash mismatch
- ed25519 verify instruction missing or mismatched

## Notes for Implementation

- PDA derivation for `provider_signer_pda` should hash `provider_code` to a fixed `[u8; 32]` seed component
- PDA derivation for `receipt_pda` should hash `request_nonce` to a fixed `[u8; 32]` seed component
- `charge_atomic` is frozen to `u64` in Phase 1; if larger domains become necessary later, add a new instruction version instead of silently changing the type
- if the team prefers to avoid a full CBOR implementation on-chain, Phase 1 can move canonical rebuild into the off-chain service only, but that materially weakens the trust boundary and is not recommended
