# Phase 1 Compact Receipt Gateway Design

Status: Draft for review
Date: 2026-04-21
Audience: airouter / Clawfarm gateway engineers
Scope: Replace the current string-heavy Phase 1 receipt submission ABI with a
production-safe compact contract that testnet and mainnet will both use.

## 1. Problem Statement

The current `submit_receipt` ABI is not production-safe because it pushes too
many variable-length business strings on chain in the same transaction that also
carries the required `ed25519` verification instruction.

This is no longer a theoretical issue. On devnet we already hit the Solana
transaction size ceiling during the negative-path smoke test:

- observed failure: `Transaction too large: 1252 > 1232`
- immediate trigger: long string fields such as `request_nonce`, `proof_id`, and
  `model` inflated the instruction data
- root cause: the ABI itself allows transaction size to scale with business
  string length

This means the current interface cannot be saved by asking clients to "keep
strings short". The protocol shape must change so the transaction size is
bounded by design.

## 2. Final Decisions

The compact receipt redesign is based on the following decisions.

- No V1 compatibility path is kept. Testnet moves directly to the production
  contract shape.
- `submit_receipt` sends fixed-size values only.
- Long business metadata stays off chain and is bound by hashes.
- `receipt_hash` becomes the primary external receipt identifier for Clawfarm
  UI, gateway APIs, and challenge entry.
- `request_nonce_hash` remains the replay-lock key used to derive the receipt
  PDA.
- `provider_code` leaves the on-chain admin and receipt ABI.
- `charge_mint` leaves the receipt ABI and the masterpool CPI args. Settlement
  mint selection is derived from the bound `masterpool` config instead of being
  user-supplied.
- The provider signer registry is keyed by `(provider_wallet, signer)` instead
  of `(provider_code, signer)`.

## 3. Target On-Chain Contract

### 3.1 Provider signer admin contract

The provider signer registry becomes wallet-native and no longer depends on any
free-form provider string.

```rust
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_wallet: Pubkey,
    signer: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()>

pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    provider_wallet: Pubkey,
    signer: Pubkey,
) -> Result<()>
```

New signer PDA seeds:

```text
["provider_signer", provider_wallet, signer]
```

`ProviderSigner` must persist the signer pubkey so `submit_receipt` can verify
`ed25519` without receiving a separate `signer` arg.

```rust
#[account]
#[derive(InitSpace)]
pub struct ProviderSigner {
    pub provider_wallet: Pubkey,
    pub signer: Pubkey,
    pub attester_type_mask: u8,
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
}
```

`provider_code` is no longer part of the registry PDA, events, or receipt args.
If the business still needs a provider code, it moves into off-chain metadata
and is covered by `metadata_hash`.

### 3.2 Compact `SubmitReceiptArgs`

The production receipt ABI becomes fixed-size only.

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SubmitReceiptArgs {
    pub request_nonce_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub charge_atomic: u64,
    pub receipt_hash: [u8; 32],
}
```

Borsh payload size is deterministic:

- `32 + 32 + 8 + 8 + 8 + 32 = 120` bytes
- instruction data becomes `8` bytes of Anchor discriminator plus `120` bytes of
  args, so `128` bytes total

That removes all transaction-size variance caused by long business strings.
Once the account list is fixed, the receipt submit transaction size is fixed too.

### 3.3 Fields removed from the on-chain submit ABI

The following fields are removed from `SubmitReceiptArgs`:

- all strings:
  - `proof_id`
  - `request_nonce`
  - `provider`
  - `model`
  - `provider_request_id`
- redundant or low-signal fields:
  - `total_tokens`
  - `http_status`
  - `latency_ms`
- user-supplied identity or mint fields:
  - `charge_mint`
  - `signer`
  - `provider_wallet`
  - `payer_user`
- protocol constants that no longer need to be transported per receipt:
  - `version`
  - `proof_mode`
  - `usage_basis`
  - `attester_type`

The contract derives the removed identity and mint values from accounts or
config:

- `provider_wallet` and `signer` come from `provider_signer`
- `payer_user` comes from the `payer_user` signer account
- `usdc_mint` comes from `masterpool` config validation

If the business wants to preserve any removed semantic field for audit or user
support, it must live in off-chain metadata and therefore inside
`metadata_hash`.

### 3.4 Compact receipt validation flow

At submit time the contract performs the following checks.

1. Validate `provider_signer` is active and inside its validity window.
2. Read `provider_wallet` and `signer` from `provider_signer`.
3. Read `payer_user` from the passed signer account.
4. Read `usdc_mint` from the configured `masterpool` accounts.
5. Rebuild the compact receipt hash preimage from:
   - `request_nonce_hash`
   - `metadata_hash`
   - `provider_wallet`
   - `payer_user`
   - `usdc_mint`
   - `prompt_tokens`
   - `completion_tokens`
   - `charge_atomic`
6. Require `sha256(preimage) == receipt_hash`.
7. Verify the preceding `ed25519` instruction signed the raw `receipt_hash` with
   `provider_signer.signer`.
8. Create the receipt PDA using `request_nonce_hash` as the replay-lock seed.
9. Forward settlement to `masterpool` without any user-supplied `charge_mint`.

## 4. Hashing Rules

The gateway must compute three hashes.

### 4.1 `request_nonce_hash`

`request_nonce_hash` is a replay-lock key, not a user-facing identifier.

```text
request_nonce_hash = sha256(utf8(raw_request_nonce))
```

Rules:

- the raw request nonce stays off chain
- the raw request nonce must still be stored by the gateway for audit and
  support tooling
- the receipt PDA continues to use `request_nonce_hash`

### 4.2 `metadata_hash`

`metadata_hash` binds the verbose business metadata that no longer travels on
chain.

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

Canonicalization rules:

- encode metadata with RFC 8785 JSON Canonicalization Scheme (JCS)
- omit absent optional fields entirely instead of serializing `null`
- use UTF-8 bytes of the canonical JSON string
- hash the canonical bytes

```text
metadata_hash = sha256(utf8(jcs(metadata_object)))
```

Because `metadata_hash` is always `32` bytes on chain, the gateway can keep the
metadata object as rich as needed without affecting transaction size.

### 4.3 `receipt_hash`

`receipt_hash` is the primary external receipt identifier.

Use the following exact compact preimage layout:

```text
receipt_hash_preimage_v2 =
  "clawfarm:receipt:v2"                     // UTF-8 domain separator
  || request_nonce_hash                     // 32 bytes
  || metadata_hash                          // 32 bytes
  || provider_wallet                        // 32 bytes
  || payer_user                             // 32 bytes
  || usdc_mint                              // 32 bytes
  || prompt_tokens_le_u64                   // 8 bytes
  || completion_tokens_le_u64               // 8 bytes
  || charge_atomic_le_u64                   // 8 bytes

receipt_hash = sha256(receipt_hash_preimage_v2)
```

Notes:

- all pubkeys are raw `32` byte Solana pubkey bytes
- integer fields are unsigned `u64` encoded in little-endian order
- the gateway must source `usdc_mint` from the deployment record or live chain
  config, never from end-user input
- the on-chain program must rebuild the same preimage and compare against the
  submitted `receipt_hash`

### 4.4 Gateway-side reference pseudocode

```ts
const requestNonceHash = sha256(Buffer.from(rawRequestNonce, "utf8"));

const metadataObject = {
  schema: "clawfarm-receipt-metadata/v2",
  proof_id,
  provider_code,
  model,
  ...(provider_request_id ? { provider_request_id } : {}),
  ...(issued_at !== undefined ? { issued_at } : {}),
  ...(expires_at !== undefined ? { expires_at } : {}),
  attester_type: "gateway",
  usage_basis: "provider_reported",
};
const metadataHash = sha256(Buffer.from(jcsCanonicalize(metadataObject), "utf8"));

const receiptHash = sha256(
  Buffer.concat([
    Buffer.from("clawfarm:receipt:v2", "utf8"),
    requestNonceHash,
    metadataHash,
    providerWallet.toBuffer(),
    payerUser.toBuffer(),
    usdcMint.toBuffer(),
    u64Le(promptTokens),
    u64Le(completionTokens),
    u64Le(chargeAtomic),
  ])
);
```

## 5. Off-Chain Receipt Record

The gateway should persist a receipt record that contains both the raw business
values and the compact hashes.

Recommended stored object:

```json
{
  "schema": "clawfarm-receipt-record/v2",
  "raw_request_nonce": "...",
  "metadata": {
    "schema": "clawfarm-receipt-metadata/v2",
    "proof_id": "...",
    "provider_code": "...",
    "model": "..."
  },
  "provider_wallet": "...",
  "payer_user": "...",
  "usdc_mint": "...",
  "prompt_tokens": 123,
  "completion_tokens": 456,
  "charge_atomic": "10000000",
  "request_nonce_hash": "0x...",
  "metadata_hash": "0x...",
  "receipt_hash": "0x...",
  "attestation_signer": "...",
  "submit_signature": "...",
  "receipt_pda": "..."
}
```

This record becomes the authoritative bridge between:

- user support
- dispute tooling
- gateway audit logs
- website receipt pages
- on-chain dispute execution

## 6. Gateway Submission Pipeline

The gateway should follow this exact flow.

1. Receive the raw provider receipt and normalized settlement inputs.
2. Resolve the configured `provider_wallet`, `signer`, and `usdc_mint` from
   Clawfarm-controlled config, not from caller input.
3. Compute `request_nonce_hash` from the raw request nonce.
4. Build the canonical metadata object and compute `metadata_hash`.
5. Build the compact receipt preimage and compute `receipt_hash`.
6. Sign the raw `32` byte `receipt_hash` with the configured provider signer.
7. Submit `submit_receipt` with the compact `SubmitReceiptArgs`.
8. Persist the off-chain receipt record.
9. Return `receipt_hash` and transaction signature as the external success
   result.

Recommended gateway success payload:

```json
{
  "receipt_hash": "0x...",
  "submit_signature": "...",
  "challenge_deadline": 1760007200,
  "receipt_pda": "..."
}
```

`receipt_pda` may be returned for internal tooling, but `receipt_hash` is the
main identifier exposed to users.

## 7. Query and Challenge Contract

### 7.1 User-facing lookup key

The user-facing lookup key is `receipt_hash` only.

Users do not need to know or submit:

- raw `request_nonce`
- `proof_id`
- `provider_request_id`
- `provider_code`
- receipt PDA

### 7.2 How Clawfarm queries by `receipt_hash`

The receipt account already stores `receipt_hash` as its first payload field, so
it is directly chain-queryable.

For direct RPC lookup, the gateway or website can call `getProgramAccounts` on
`clawfarm-attestation` with a memcmp filter at offset `8`:

- first `8` bytes: Anchor account discriminator
- next `32` bytes: `Receipt.receipt_hash`

Pseudo-query flow:

```ts
const matches = await connection.getProgramAccounts(attestationProgramId, {
  filters: [
    {
      memcmp: {
        offset: 8,
        bytes: bs58.encode(receiptHashBytes),
      },
    },
  ],
});
```

Clawfarm can therefore accept only `receipt_hash` from the user, resolve the
matching receipt account from chain data or the internal index, and then build
challenge transactions with the resolved receipt PDA.

### 7.3 Challenge API contract

The website / gateway challenge entry should accept:

```json
{
  "receipt_hash": "0x...",
  "challenge_type": "payload_mismatch",
  "evidence_hash": "0x..."
}
```

The gateway then:

1. resolves `receipt_hash -> receipt_pda`
2. fetches the off-chain receipt record by `receipt_hash`
3. prepares evidence
4. submits `open_challenge`

## 8. Masterpool Impact

`masterpool::record_mining_from_receipt` should stop receiving a user-supplied
`charge_mint` argument.

Current shape:

```rust
pub struct RecordMiningFromReceiptArgs {
    pub total_usdc_paid: u64,
    pub charge_mint: Pubkey,
}
```

Target shape:

```rust
pub struct RecordMiningFromReceiptArgs {
    pub total_usdc_paid: u64,
}
```

Mint safety still remains strict because:

- `usdc_mint` account is already constrained to `config.usdc_mint`
- payer token accounts are already validated against `config.usdc_mint`
- receipt submission no longer allows callers to inject an arbitrary mint arg

So the protocol becomes safer and simpler at the same time.

## 9. Migration Mapping From Current Payload

| Current field | New location |
| --- | --- |
| `proof_id` | `metadata.proof_id` |
| `request_nonce` | off-chain raw field + `request_nonce_hash` |
| `provider` | `metadata.provider_code` |
| `model` | `metadata.model` |
| `provider_request_id` | `metadata.provider_request_id` |
| `issued_at` | `metadata.issued_at` |
| `expires_at` | `metadata.expires_at` |
| `total_tokens` | derived off chain, not submitted |
| `http_status` | removed |
| `latency_ms` | removed |
| `charge_mint` | derived from config, not submitted |
| `signer` | `ProviderSigner.signer` |
| `provider_wallet` | `ProviderSigner.provider_wallet` |
| `payer_user` | `payer_user` account |
| `receipt_hash` | unchanged, still the trust anchor |

## 10. Acceptance Criteria

The redesign is correct only if all of the following are true.

- `submit_receipt` instruction data size is fixed regardless of `proof_id`,
  `model`, or other business string lengths.
- The gateway can submit receipts whose off-chain metadata is arbitrarily long
  without crossing the Solana transaction size limit.
- The contract rejects any attempt to use a mint other than the configured test
  or production USDC mint.
- Clawfarm website and gateway can look up a receipt using `receipt_hash` alone.
- The dispute flow can be initiated with `receipt_hash` plus `evidence_hash`
  only.
- Testnet and mainnet use the same compact ABI.

## 11. Recommended Gateway Work Items

airouter / Clawfarm gateway should implement the following changes together.

1. Replace the current submit payload builder with the compact hash pipeline.
2. Store raw receipt metadata off chain and persist the hashed receipt record.
3. Standardize user-facing receipt IDs on `receipt_hash`.
4. Add `receipt_hash -> receipt_pda` lookup through chain memcmp or internal
   indexing.
5. Remove any caller-controlled `charge_mint` from the submission path.
6. Switch signer-registry configuration to `(provider_wallet, signer)`.

## 12. Non-Goals

This redesign does not add:

- a V1 fallback path
- on-chain storage of long receipt metadata
- challenge submission by raw `request_nonce`
- a generic multi-token settlement model
- on-chain mint rotation

## 13. Recommendation

Adopt this compact receipt contract as the only supported Phase 1 ABI and make
both testnet and mainnet gateway integrations target it directly.
