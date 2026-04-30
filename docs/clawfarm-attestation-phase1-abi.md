# Clawfarm Attestation Phase 1 ABI

Status: Implemented
Version: v2 compact receipt contract
Last Updated: 2026-04-21

This document describes the current on-chain ABI of `clawfarm_attestation` in
this repository.

Source of truth:

- `programs/clawfarm-attestation/src/lib.rs`
- `programs/clawfarm-attestation/src/instructions/admin.rs`
- `programs/clawfarm-attestation/src/instructions/receipt.rs`
- `programs/clawfarm-attestation/src/instructions/challenge.rs`
- `programs/clawfarm-attestation/src/state/accounts.rs`
- `programs/clawfarm-attestation/src/state/types.rs`
- `programs/clawfarm-attestation/src/events.rs`

## Phase 1 Contract Shape

Phase 1 uses a compact fixed-size receipt ABI:

```rust
pub struct SubmitReceiptArgs {
    pub request_nonce_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub charge_atomic: u64,
    pub receipt_hash: [u8; 32],
}
```

Key properties:

- `request_nonce_hash` is the replay-lock seed.
- `metadata_hash` binds rich off-chain receipt metadata.
- `receipt_hash` is the primary external receipt identifier.
- `provider_wallet`, `payer_user`, `signer`, and `usdc_mint` are derived from
  accounts or config, not passed as user-controlled receipt args.
- long metadata never increases instruction size because only hashes go on
  chain.

## PDA Seeds

```text
CONFIG_SEED          = "config"
PROVIDER_SIGNER_SEED = "provider_signer"
RECEIPT_SEED         = "receipt"
CHALLENGE_SEED       = "challenge"
```

Runtime derivations:

```text
Config         = ["config"]
ProviderSigner = ["provider_signer", provider_wallet, signer]
Receipt        = ["receipt", request_nonce_hash]
Challenge      = ["challenge", receipt.key()]
```

## Account Layouts

### `Config`

```text
authority: Pubkey
pause_authority: Pubkey
challenge_resolver: Pubkey
masterpool_program: Pubkey
challenge_window_seconds: i64
challenge_resolution_timeout_seconds: i64
is_paused: bool
```

### `ProviderSigner`

```text
signer: Pubkey
provider_wallet: Pubkey
attester_type_mask: u8
status: u8
valid_from: i64
valid_until: i64
```

### `Receipt`

```text
receipt_hash: [u8; 32]
signer: Pubkey
payer_user: Pubkey
provider_wallet: Pubkey
submitted_at: i64
challenge_deadline: i64
finalized_at: i64
status: u8
economics_settled: bool
```

### `Challenge`

```text
receipt: Pubkey
challenger: Pubkey
challenge_type: u8
evidence_hash: [u8; 32]
bond_amount: u64
opened_at: i64
resolved_at: i64
status: u8
resolution_code: u8
```

## Enums

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

## Instruction ABI

### `initialize_config`

```rust
pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    masterpool_program: Pubkey,
    challenge_window_seconds: i64,
    challenge_resolution_timeout_seconds: i64,
) -> Result<()>
```

Rules:

- all authority pubkeys must be non-zero
- `masterpool_program` must be non-zero
- both windows must be positive
- initializer must be the current upgrade authority via `ProgramData`

### `upsert_provider_signer`

```rust
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    signer: Pubkey,
    provider_wallet: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()>
```

Rules:

- PDA seed is `("provider_signer", provider_wallet, signer)`
- `signer` and `provider_wallet` must be non-zero
- `attester_type_mask` must be non-zero
- `valid_until == 0 || valid_until >= valid_from`

### `set_pause`

```rust
pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()>
```

### `revoke_provider_signer`

```rust
pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    signer: Pubkey,
    provider_wallet: Pubkey,
) -> Result<()>
```

### `submit_receipt`

```rust
pub fn submit_receipt(
    ctx: Context<SubmitReceipt>,
    args: SubmitReceiptArgs,
) -> Result<()>
```

Rules:

- receipt PDA seed is `request_nonce_hash`
- signer registry entry must be active and within validity window
- signer registry must include the gateway attester bit for Phase 1 gateway flow
- program rebuilds the compact receipt hash from:
  - `request_nonce_hash`
  - `metadata_hash`
  - `provider_signer.provider_wallet`
  - `payer_user.key()`
  - `usdc_mint.key()`
  - `prompt_tokens`
  - `completion_tokens`
  - `charge_atomic`
- rebuilt hash must equal `receipt_hash`
- preceding `ed25519` instruction must verify `receipt_hash` with
  `provider_signer.signer`
- economics are forwarded to masterpool by CPI using config-bound USDC accounts

### `open_challenge`

```rust
pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()>
```

### `resolve_challenge`

```rust
pub fn resolve_challenge(
    ctx: Context<ResolveChallenge>,
    resolution_code: u8,
) -> Result<()>
```

### `timeout_reject_challenge`

```rust
pub fn timeout_reject_challenge(
    ctx: Context<TimeoutRejectChallenge>,
) -> Result<()>
```

### `finalize_receipt`

```rust
pub fn finalize_receipt(ctx: Context<FinalizeReceipt>) -> Result<()>
```

### `close_challenge`

```rust
pub fn close_challenge(ctx: Context<CloseChallenge>) -> Result<()>
```

### `close_receipt`

```rust
pub fn close_receipt(ctx: Context<CloseReceipt>) -> Result<()>
```

## Events

### `ProviderSignerUpserted`

```text
signer
provider_wallet
attester_type_mask
```

### `ProviderSignerRevoked`

```text
signer
provider_wallet
```

### `ReceiptSubmitted`

```text
receipt
request_nonce_hash
metadata_hash
provider_wallet
payer_user
signer
receipt_hash
challenge_deadline
```

### `ReceiptFinalized`

```text
receipt
signer
receipt_hash
```

### `ReceiptClosed`

```text
receipt
signer
receipt_hash
status
```

## Receipt Lookup Contract

- primary external receipt id: `receipt_hash`
- replay-lock seed: `request_nonce_hash = sha256(utf8(raw_request_nonce))`
- chain lookup: memcmp on `Receipt.receipt_hash` at offset `8`
- off-chain metadata lookup: by `receipt_hash` or by retained raw request nonce
  inside gateway storage
