# clawfarm-attestation

`clawfarm-attestation` is the dedicated Solana receipt lifecycle program for
Clawfarm Phase 1.

Chinese version:

- [README.zh-CN.md](README.zh-CN.md)

This README reflects the current implementation in this repository.

Source of truth:

- [src/lib.rs](src/lib.rs)
- [src/instructions/admin.rs](src/instructions/admin.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [src/events.rs](src/events.rs)
- [../clawfarm-masterpool/README.md](../clawfarm-masterpool/README.md)

## Responsibilities

- maintain the provider signer registry
- verify compact receipt hashes against runtime identities
- prevent replay with `request_nonce_hash`
- manage receipt and challenge lifecycle state
- forward economics to `clawfarm-masterpool` through CPI
- close terminal receipt and challenge accounts to reclaim rent

## Compact Receipt Model

Phase 1 uses a fixed-size receipt payload:

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

Why this shape exists:

- verbose metadata stays off chain behind `metadata_hash`
- raw request nonce stays off chain behind `request_nonce_hash`
- `receipt_hash` becomes the primary external receipt identifier
- instruction size stays bounded even when business metadata is large

The compact hash preimage is:

```text
"clawfarm:receipt:v2"
|| request_nonce_hash
|| metadata_hash
|| provider_wallet
|| payer_user
|| usdc_mint
|| prompt_tokens_le_u64
|| completion_tokens_le_u64
|| charge_atomic_le_u64
```

The program rebuilds this digest on chain and verifies the preceding `ed25519`
instruction against `provider_signer.signer`.

## Program State

### `Config`

Seed:

- `("config")`

Fields:

- `authority`
- `pause_authority`
- `challenge_resolver`
- `masterpool_program`
- `challenge_window_seconds`
- `challenge_resolution_timeout_seconds`
- `is_paused`

### `ProviderSigner`

Seed:

- `("provider_signer", provider_wallet, signer)`

Fields:

- `signer`
- `provider_wallet`
- `attester_type_mask`
- `status`
- `valid_from`
- `valid_until`

### `Receipt`

Seed:

- `("receipt", request_nonce_hash)`

Stored fields:

- `receipt_hash`
- `signer`
- `payer_user`
- `provider_wallet`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `status`
- `economics_settled`

### `Challenge`

Seed:

- `("challenge", receipt.key())`

Stored fields:

- `receipt`
- `challenger`
- `challenge_type`
- `evidence_hash`
- `bond_amount`
- `opened_at`
- `resolved_at`
- `status`
- `resolution_code`

## Instruction Surface

- `initialize_config(...)`
  - requires the current upgrade authority through `ProgramData`
  - stores governance authorities and linked masterpool program id
- `upsert_provider_signer(signer, provider_wallet, attester_type_mask, valid_from, valid_until)`
  - creates or updates a signer policy record keyed by wallet plus signer
- `set_pause(is_paused)`
  - toggles receipt submission pause state
- `revoke_provider_signer(signer, provider_wallet)`
  - marks the signer policy as revoked
- `submit_receipt(args)`
  - validates compact hash, signer policy, and preceding `ed25519`
  - records receipt lifecycle state
  - CPIs into masterpool to record economics
- `open_challenge(challenge_type, evidence_hash)`
  - opens one challenge slot for one receipt and records bond economics in
    masterpool
- `resolve_challenge(resolution_code)`
  - resolves an open challenge and triggers challenge economics in masterpool
- `timeout_reject_challenge()`
  - lets authority reject stale unresolved challenges after timeout
- `finalize_receipt()`
  - finalizes eligible receipts and settles provider payout in masterpool
- `close_challenge()` / `close_receipt()`
  - reclaim rent after terminal lifecycle completion

## Integration Notes

- gateway signer authorization is enforced through `attester_type_mask`
- gateway receipt lookup should use `receipt_hash`
- chain lookup can memcmp `Receipt.receipt_hash` at offset `8`
- off-chain systems should retain:
  - raw request nonce
  - full metadata object
  - `receipt_hash`
  - provider-side request identifiers used for support and disputes

## Current Constraints

- one `Receipt` PDA exists per `request_nonce_hash`
- one `Challenge` PDA exists per `Receipt`
- if a challenge is rejected, the receipt still needs `finalize_receipt` before
  provider payout is settled in masterpool
- `close_receipt` requires both a terminal receipt state and
  `economics_settled == true`
- challenge bond, slash, reward, and burn economics all live in masterpool, not
  in this program
