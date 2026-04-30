# Clawfarm Attestation Phase 1 Contract Interface Design

Status: Implemented
Version: v2 compact receipt contract
Last Updated: 2026-04-21

This document explains the current Phase 1 interface and runtime logic of the
`clawfarm_attestation` program.

Source of truth:

- `programs/clawfarm-attestation/src/lib.rs`
- `programs/clawfarm-attestation/src/instructions/admin.rs`
- `programs/clawfarm-attestation/src/instructions/receipt.rs`
- `programs/clawfarm-attestation/src/instructions/challenge.rs`
- `programs/clawfarm-attestation/src/state/accounts.rs`
- `programs/clawfarm-attestation/src/state/types.rs`
- `programs/clawfarm-attestation/src/events.rs`

## Design Goals

Phase 1 keeps attestation narrow and production-safe:

- verify authorized provider or gateway signers on chain
- anchor each paid receipt with a fixed-size compact digest contract
- prevent replay with `request_nonce_hash`
- support governance-driven challenge and timeout flows
- keep economics in `clawfarm-masterpool`, not in this program

Phase 1 does not store verbose receipt metadata on chain.

## Compact Receipt Contract

The on-chain receipt payload is fixed-size only:

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

Design consequences:

- long business fields stay off chain behind `metadata_hash`
- transaction size no longer scales with metadata length
- `receipt_hash` is the main external receipt id for gateway, UI, and challenge
  tooling
- `provider_wallet`, `payer_user`, and `usdc_mint` are bound from accounts and
  config, not user-supplied inside receipt args

## Hashing Model

### `request_nonce_hash`

```text
request_nonce_hash = sha256(utf8(raw_request_nonce))
```

Purpose:

- replay lock seed for the `Receipt` PDA
- off-chain systems must still retain the raw request nonce for support and
  auditing

### `metadata_hash`

`metadata_hash` binds the off-chain business metadata that is too large or too
variable to carry on chain.

Recommended metadata object:

```json
{
  "schema": "clawfarm-receipt-metadata/v2",
  "proof_id": "...",
  "provider_code": "...",
  "model": "...",
  "provider_request_id": "...",
  "issued_at": 1760000000,
  "expires_at": 1760000300,
  "attester_type": "gateway",
  "usage_basis": "provider_reported"
}
```

Rules:

- encode canonical JSON before hashing
- omit absent optional fields instead of serializing `null`
- retain the full metadata object off chain for challenge review and user
  support

### `receipt_hash`

The program rebuilds the receipt digest from the compact preimage:

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

Then:

```text
receipt_hash = sha256(preimage)
```

## Provider Signer Registry

Provider signer records are keyed by wallet plus signer pubkey, not by a free
form provider code.

PDA seed:

```text
["provider_signer", provider_wallet, signer]
```

Stored fields:

```text
signer
provider_wallet
attester_type_mask
status
valid_from
valid_until
```

Why this matters:

- the same signer pubkey can be scoped to a specific provider wallet
- `submit_receipt` can read `provider_wallet` and `signer` directly from the
  signer record
- gateway authorization is enforced through `attester_type_mask`

## Receipt Flow

### Submit

`submit_receipt` performs the following checks:

1. config is not paused
2. provider signer is active
3. provider signer validity window includes `now`
4. provider signer includes the gateway attester bit
5. compact hash rebuilt from runtime accounts equals submitted `receipt_hash`
6. preceding `ed25519` instruction signed the raw `receipt_hash` with
   `provider_signer.signer`
7. linked masterpool provider account belongs to the same provider wallet
8. masterpool CPI records receipt economics using config-bound USDC accounts

Stored `Receipt` fields are intentionally minimal:

```text
receipt_hash
signer
payer_user
provider_wallet
submitted_at
challenge_deadline
finalized_at
status
economics_settled
```

### Finalize

If no challenge blocks completion and the challenge window has expired,
`finalize_receipt` marks the attestation receipt finalized and triggers the
masterpool settlement CPI.

### Close

`close_receipt` only succeeds when:

- receipt status is terminal, and
- `economics_settled == true`

This prevents rent cleanup from skipping economic settlement.

## Challenge Flow

Each receipt has at most one `Challenge` PDA:

```text
["challenge", receipt.key()]
```

Flow:

1. challenger opens a challenge and masterpool records the `CLAW` bond
2. resolver accepts or rejects the challenge, or authority later times it out as
   rejected
3. accepted challenges lead to masterpool challenge-economic resolution
4. rejected challenges still require `finalize_receipt` if provider payout has
   not yet been settled

## Events and Indexing

The compact interface emits hash-first events.

`ReceiptSubmitted` exposes:

- `receipt`
- `request_nonce_hash`
- `metadata_hash`
- `provider_wallet`
- `payer_user`
- `signer`
- `receipt_hash`
- `challenge_deadline`

Indexing guidance:

- primary lookup key: `receipt_hash`
- receipt account query: memcmp on `Receipt.receipt_hash` at offset `8`
- gateway should map `receipt_hash` back to retained off-chain metadata and raw
  request nonce

## Integration Boundary with Masterpool

This program owns:

- signer registry policy
- receipt lifecycle state
- challenge lifecycle state
- compact hash verification

`clawfarm-masterpool` owns:

- fixed `CLAW` and fixed settlement-mint binding
- provider staking and exit rules
- USDC treasury and provider pending vaults
- reward accounting, release, claim, bond, slash, and burn logic

The attestation program never accepts an arbitrary settlement mint from user
receipt args.
