# clawfarm-attestation

`clawfarm-attestation` is a dedicated Solana program for Clawfarm Phase 1
receipt attestation.

Chinese version:

- [README.zh-CN.md](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/README.zh-CN.md)

Its responsibilities are:

- maintain a provider signer registry
- verify provider signatures over canonical receipt digests
- prevent replay by `request_nonce`
- manage a governance-driven challenge lifecycle
- close terminal receipt and challenge accounts to reclaim rent

This README documents the current on-chain implementation in this repository.

## High-Level Model

Phase 1 uses a minimal on-chain receipt anchor.

The full receipt body is expected to exist off-chain, for example in Clawfarm
managed S3 storage. The program only keeps:

- `receipt_hash`
- `signer`
- lifecycle timestamps
- receipt status

The trust boundary is:

1. the full receipt body is canonicalized off-chain
2. the program rebuilds the same canonical payload on-chain
3. the program checks `sha256(canonical_payload) == receipt_hash`
4. the program checks the preceding `ed25519` verify instruction
5. the program stores a minimal receipt anchor keyed by `request_nonce`

## Program State

State definitions live in [accounts.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/state/accounts.rs).

### `Config`

PDA seed:

- `["config"]`

Fields:

- `authority`
- `pause_authority`
- `challenge_resolver`
- `challenge_window_seconds`
- `receipt_count`
- `challenge_count`
- `is_paused`
- `phase2_enabled`
- `bump`
- `reserved`

Purpose:

- global governance and timing config

### `ProviderSigner`

PDA seed:

- `["provider_signer", sha256(provider_code), signer_pubkey]`

Fields:

- `provider_code`
- `signer`
- `key_id`
- `attester_type_mask`
- `status`
- `valid_from`
- `valid_until`
- `metadata_hash`
- `created_at`
- `updated_at`
- `bump`
- `reserved`

Purpose:

- on-chain signer registry for providers or gateways

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
- `bump`

Purpose:

- replay lock keyed by `request_nonce`
- anchor for an off-chain receipt body
- state machine for challenge and close

### `Challenge`

PDA seed:

- `["challenge", receipt.key(), challenge_type, challenger.key()]`

Fields:

- `receipt`
- `challenger`
- `challenge_type`
- `evidence_hash`
- `opened_at`
- `resolved_at`
- `status`
- `resolution_code`
- `bump`

Purpose:

- one dispute instance against one receipt and one challenge type

## Enum Values

Definitions live in [types.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/state/types.rs).

### `ProofMode`

- `0 = SigLog`
- `1 = SigLogZkReserved`

Phase 1 only accepts `SigLog`.

### `AttesterType`

- `0 = Provider`
- `1 = Gateway`
- `2 = Hybrid`

### `UsageBasis`

- `0 = ProviderReported`
- `1 = ServerEstimatedReserved`
- `2 = HybridReserved`
- `3 = TokenizerVerifiedReserved`

Phase 1 only accepts `ProviderReported`.

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
- `3 = Expired`

Note:

- `Expired` is reserved in the enum but not currently written by any instruction

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

## Canonical Receipt Contract

The canonicalization logic lives in [utils.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/utils.rs).

Rules:

- the program receives structured `SubmitReceiptArgs`
- the program rebuilds a deterministic CBOR payload on-chain
- optional fields are omitted when absent
- `receipt_hash = sha256(canonical_cbor_bytes)`
- the preceding transaction instruction must be a matching `ed25519`
  verification over the raw 32-byte digest

Fields intentionally excluded from the signed payload:

- `proof_url`
- `signer`
- `signature`
- `receipt_hash`

That means `proof_url` is validated as a transport field, but it is not stored in
the receipt account and not part of the canonical hash in Phase 1.

## Recommended Clawfarm S3 Flow

This repository does not implement the off-chain storage service, but the current
contract is designed to support the following operational flow.

### Actors

- `Provider`: returns the raw usage receipt body
- `Clawfarm service`: canonicalizes the receipt, uploads it to S3, and submits
  the on-chain transaction
- `Clawfarm website`: exposes a lookup entry for users, challengers, and support
- `clawfarm-attestation`: verifies the digest and manages lifecycle state

### Recommended flow

1. Provider returns the full receipt payload to Clawfarm.
2. Clawfarm validates the payload shape and normalizes it into the canonical
   Phase 1 schema.
3. Clawfarm computes canonical CBOR and `receipt_hash`.
4. Clawfarm stores the full canonical receipt in S3.
5. Clawfarm stores metadata in its own index, such as:
   - `receipt_hash`
   - `request_nonce`
   - `provider`
   - `proof_id`
   - S3 object key
   - submission status
6. Clawfarm creates the `ed25519` verify instruction and sends `submit_receipt`.
7. The website exposes a search entry by `receipt_hash`, `request_nonce`, or
   Clawfarm internal id.
8. During challenge, Clawfarm or a challenger retrieves the stored object from
   S3, reconstructs the evidence package, and submits only evidence hashes
   on-chain.
9. After the receipt reaches terminal state, Clawfarm closes `Challenge` and
   `Receipt` to reclaim rent.

### Suggested S3 object layout

- `receipts/{provider}/{yyyy}/{mm}/{receipt_hash}.json`
- or `receipts/{receipt_hash}.cbor`

### Suggested off-chain index fields

- `receipt_hash`
- `request_nonce`
- `provider`
- `proof_id`
- `signer`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `receipt_status`
- `challenge_status`
- `s3_bucket`
- `s3_key`
- `content_type`
- `schema_version`

### Operational notes

- the trust anchor remains the on-chain `receipt_hash`, not the S3 URL
- S3 objects should be treated as immutable after upload
- if possible, enable bucket versioning and disallow overwrite
- the website should read from the Clawfarm index, not derive state only from
  S3 object listing
- the close flow should run only after terminal state is confirmed on-chain

## Resolver Bot Flow

The intended `challenge_resolver` is an automated Clawfarm bot rather than a
human-operated wallet.

Recommended loop:

1. watch for newly opened `ChallengeOpened` events or poll challenge PDAs whose
   status is still `Open`
2. load the referenced receipt from the Clawfarm index and fetch the full
   canonical receipt plus challenge evidence from off-chain storage
3. reconstruct the dispute package off-chain and run Clawfarm-specific
   verification logic for the requested `challenge_type`
4. derive a single `resolution_code` from that verification result:
   - `Rejected` if the challenge is not valid
   - `Accepted` or `ReceiptInvalidated` if the receipt is invalid
   - `SignerRevoked` if the signer should be slashed and revoked
5. submit `resolve_challenge` from the bot-controlled `challenge_resolver`
   authority
6. after the receipt and challenge are terminal, run the rent-reclaim flow with
   `close_challenge` and `close_receipt`

Operational recommendations:

- keep the resolver bot stateless on-chain; the durable source of truth should
  remain the Clawfarm index plus the on-chain receipt/challenge PDAs
- make off-chain verification deterministic and replayable so a later audit can
  explain why a specific `resolution_code` was chosen
- use idempotent job scheduling; the bot should safely retry if an RPC call or
  evidence fetch fails midway
- record the fetched evidence object version or content hash in the bot logs so
  operators can trace the exact material used for a decision

## Rent Estimate

The current implementation minimizes long-lived cost by keeping only `ReceiptLite`
on-chain and closing terminal accounts as soon as possible.

### Current account sizes

- `Receipt` allocated size: `98 bytes`
- `Challenge` allocated size: `124 bytes`

### Rent formula

Using the current Solana rent-exempt formula:

```text
minimum_balance = (account_data_len + 128) * 6,960 lamports
```

That gives:

- per `Receipt`: `(98 + 128) * 6,960 = 1,572,960 lamports = 0.00157296 SOL`
- per `Challenge`: `(124 + 128) * 6,960 = 1,753,920 lamports = 0.00175392 SOL`

Important:

- this is rent-exempt collateral, not permanent gas burn
- the lamports are returned when `close_receipt` or `close_challenge` succeeds

### Peak collateral formula

If receipts are closed after the challenge window ends, steady-state peak locked
collateral is approximately:

```text
receipt_peak_sol
  = daily_call_count * challenge_window_days * 0.00157296
```

If every receipt also has one live challenge at the same time, the conservative
upper bound is:

```text
receipt_plus_challenge_peak_sol
  = daily_call_count * challenge_window_days * 0.00332688
```

### Receipt-only peak collateral

Assuming each call creates one `Receipt` and receipts are closed after the window:

| Daily Calls | 1 Day Window | 3 Day Window | 7 Day Window |
|---|---:|---:|---:|
| 1,000 | 1.57296 SOL | 4.71888 SOL | 11.01072 SOL |
| 10,000 | 15.7296 SOL | 47.1888 SOL | 110.1072 SOL |
| 100,000 | 157.296 SOL | 471.888 SOL | 1101.072 SOL |

### Conservative upper bound with one live challenge per receipt

Assuming every receipt also has one `Challenge` account alive at the same time:

| Daily Calls | 1 Day Window | 3 Day Window | 7 Day Window |
|---|---:|---:|---:|
| 1,000 | 3.32688 SOL | 9.98064 SOL | 23.28816 SOL |
| 10,000 | 33.2688 SOL | 99.8064 SOL | 232.8816 SOL |
| 100,000 | 332.688 SOL | 998.064 SOL | 2328.816 SOL |

### Practical reading

- if challenge rate is low, real usage should stay close to the receipt-only table
- the main optimization is not lowering transaction fee, but lowering and reclaiming
  rent collateral
- shortening `challenge_window_seconds` directly lowers peak capital locked in rent

## Instruction Reference

Entry points live in [lib.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/lib.rs).

## 1. `initialize_config`

Implementation:

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L11)

Signature:

```rust
pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    challenge_window_seconds: i64,
) -> Result<()>
```

Accounts:

- `payer`: signer, pays rent for `Config`
- `config`: config PDA, initialized with seed `["config"]`
- `system_program`

Input parameters:

- `authority`: main governance authority
- `pause_authority`: authority allowed to toggle pause
- `challenge_resolver`: resolver authority, typically an automated Clawfarm bot, allowed to resolve disputes
- `challenge_window_seconds`: receipt challenge window, must be `> 0`

Function flow:

1. checks the challenge window value is positive
2. initializes the config PDA
3. writes all governance addresses and timing values
4. zeroes `receipt_count` and `challenge_count`
5. sets `is_paused = false`
6. sets `phase2_enabled = false`
7. records the PDA bump
8. emits `ConfigInitialized`

Result:

- a unique `Config` account exists and the program is ready for signer registry
  updates

## 2. `upsert_provider_signer`

Implementation:

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L40)

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

Accounts:

- `authority`: signer, must equal `config.authority`
- `config`: config PDA
- `provider_signer`: PDA derived from `provider_code` and `signer`
- `system_program`

Input parameters:

- `provider_code`: provider identifier
- `signer`: provider or gateway signer public key
- `key_id`: off-chain key identifier
- `attester_type_mask`: bitmask of supported attester types
- `valid_from`: signer validity start
- `valid_until`: signer validity end, `0` means open-ended
- `metadata_hash`: hash of signer metadata stored off-chain

Function flow:

1. validates `provider_code`
2. validates `key_id`
3. requires `attester_type_mask != 0`
4. requires `valid_until == 0 || valid_until >= valid_from`
5. creates or reuses the signer PDA via `init_if_needed`
6. preserves `created_at` if the account already exists
7. overwrites signer registry fields with new values
8. sets `status = Active`
9. updates timestamps and bump
10. emits `ProviderSignerUpserted`

Result:

- a signer registry entry exists and can be used by `submit_receipt`

## 3. `set_pause`

Implementation:

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L85)

Signature:

```rust
pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()>
```

Accounts:

- `pause_authority`: signer, must equal `config.pause_authority`
- `config`: config PDA

Input parameters:

- `is_paused`: target pause flag

Function flow:

1. checks pause authority through Anchor account constraints
2. writes `config.is_paused`
3. emits `PauseUpdated`

Result:

- future `submit_receipt` calls are allowed or blocked depending on the new flag

Note:

- current implementation only checks pause inside `submit_receipt`

## 4. `revoke_provider_signer`

Implementation:

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L93)

Signature:

```rust
pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    provider_code: String,
    signer: Pubkey,
) -> Result<()>
```

Accounts:

- `authority`: signer, must equal `config.authority`
- `config`: config PDA
- `provider_signer`: signer PDA for the target provider and signer

Input parameters:

- `provider_code`: provider identifier
- `signer`: signer public key to revoke

Function flow:

1. validates `provider_code`
2. checks the loaded signer account matches `provider_code`
3. checks the loaded signer account matches `signer`
4. sets status to `Revoked`
5. updates `updated_at`
6. emits `ProviderSignerRevoked`

Result:

- the signer can no longer be used for future receipt submissions

## 5. `submit_receipt`

Implementation:

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs#L16)

Signature:

```rust
pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()>
```

Accounts:

- `payer`: signer, pays rent for `Receipt`
- `config`: config PDA, mutable because `receipt_count` increments
- `provider_signer`: signer registry PDA
- `receipt`: receipt PDA derived from `request_nonce`
- `instructions_sysvar`: Solana instruction sysvar, used for `ed25519` introspection
- `system_program`

Instruction args:

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

Parameter meaning:

- `version`: must be `1`
- `proof_mode`: must be `SigLog`
- `proof_id`: provider-side proof identifier
- `request_nonce`: unique business nonce used for replay protection
- `provider`: provider code
- `attester_type`: provider, gateway, or hybrid
- `model`: off-chain model identifier
- `usage_basis`: must be `ProviderReported`
- `prompt_tokens`: input token count
- `completion_tokens`: output token count
- `total_tokens`: must equal prompt plus completion
- `charge_atomic`: fee amount in smallest unit
- `charge_mint`: fee mint
- `provider_request_id`: optional provider request identifier
- `issued_at`: optional issuance time
- `expires_at`: optional expiry time
- `http_status`: optional HTTP status
- `latency_ms`: optional request latency
- `proof_url`: validated transport field, not stored in `Receipt`
- `receipt_hash`: canonical receipt digest
- `signer`: signer public key
- `signature`: signer signature over `receipt_hash`

Function flow:

1. validates all structured fields
2. checks the program is not paused
3. loads and validates the provider signer registry entry
4. rebuilds canonical CBOR on-chain
5. hashes the canonical payload and requires it to match `receipt_hash`
6. inspects the previous transaction instruction and requires a matching
   `ed25519` verification
7. creates the `Receipt` PDA
8. stores only `receipt_hash`, `signer`, timestamps, status, and bump
9. increments `config.receipt_count`
10. emits `ReceiptSubmitted`

Result:

- one unique `Receipt` account exists for the given `request_nonce`
- the full receipt body remains off-chain but is anchored by `receipt_hash`

## 6. `open_challenge`

Implementation:

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs#L13)

Signature:

```rust
pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    request_nonce: String,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()>
```

Accounts:

- `challenger`: signer, pays rent for `Challenge`
- `config`: config PDA, mutable because `challenge_count` increments
- `receipt`: target receipt PDA
- `challenge`: challenge PDA for `(receipt, challenge_type, challenger)`
- `system_program`

Input parameters:

- `request_nonce`: used to derive the receipt PDA
- `challenge_type`: challenge category
- `evidence_hash`: hash of off-chain challenge evidence

Function flow:

1. validates `request_nonce`
2. validates `challenge_type`
3. checks the receipt is still `Submitted`
4. checks current time is within the challenge window
5. creates the `Challenge` PDA
6. stores challenger, evidence hash, timestamps, and status
7. sets the receipt status to `Challenged`
8. increments `config.challenge_count`
9. emits `ChallengeOpened`

Result:

- a dispute exists for this challenger and challenge type
- the receipt is now in `Challenged` state

## 7. `resolve_challenge`

Implementation:

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs#L55)

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

Accounts:

- `challenge_resolver`: signer, must equal `config.challenge_resolver`
- `config`: config PDA
- `receipt`: referenced receipt PDA
- `challenge`: target challenge PDA

Input parameters:

- `request_nonce`: used to derive the receipt PDA
- `challenge_type`: target challenge type
- `challenger`: original challenger
- `resolution_code`: final resolution

Function flow:

1. validates `request_nonce`
2. validates `challenge_type`
3. validates `resolution_code` and rejects `None`
4. checks the challenge matches challenger and type
5. checks the challenge is `Open`
6. writes `resolution_code` and `resolved_at`
7. updates the receipt to a terminal state:
   - `Accepted` or `ReceiptInvalidated` -> `Rejected`
   - `SignerRevoked` -> `Slashed`
   - `Rejected` -> `Finalized`
8. sets `receipt.finalized_at = now`
9. emits `ChallengeResolved`

Result:

- the receipt leaves the active dispute state and becomes closable later

## 8. `finalize_receipt`

Implementation:

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs#L84)

Signature:

```rust
pub fn finalize_receipt(ctx: Context<FinalizeReceipt>, request_nonce: String) -> Result<()>
```

Accounts:

- `caller`: any signer
- `config`: config PDA
- `receipt`: target receipt PDA

Input parameters:

- `request_nonce`: used to derive the receipt PDA

Function flow:

1. validates `request_nonce`
2. checks the receipt is still `Submitted`
3. checks `now > challenge_deadline`
4. sets receipt status to `Finalized`
5. sets `finalized_at = now`
6. emits `ReceiptFinalized`

Result:

- an uncontested receipt becomes terminal and can later be closed

## 9. `close_challenge`

Implementation:

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs#L117)

Signature:

```rust
pub fn close_challenge(
    ctx: Context<CloseChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
) -> Result<()>
```

Accounts:

- `recipient`: signer, receives reclaimed lamports
- `receipt`: receipt PDA used in the challenge seed
- `challenge`: challenge PDA to close

Input parameters:

- `request_nonce`: used to derive the receipt PDA
- `challenge_type`: target challenge type
- `challenger`: original challenger

Function flow:

1. validates `request_nonce`
2. validates `challenge_type`
3. checks challenge status is terminal:
   - `Accepted`
   - `Rejected`
   - `Expired`
4. emits `ChallengeClosed`
5. closes the challenge account through Anchor `close = recipient`

Result:

- challenge rent is returned to `recipient`

## 10. `close_receipt`

Implementation:

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs#L109)

Signature:

```rust
pub fn close_receipt(ctx: Context<CloseReceipt>, request_nonce: String) -> Result<()>
```

Accounts:

- `recipient`: signer, receives reclaimed lamports
- `receipt`: terminal receipt PDA

Input parameters:

- `request_nonce`: used to derive the receipt PDA

Function flow:

1. validates `request_nonce`
2. checks receipt status is terminal:
   - `Finalized`
   - `Rejected`
   - `Slashed`
3. emits `ReceiptClosed`
4. closes the receipt account through Anchor `close = recipient`

Result:

- receipt rent is returned to `recipient`

## Lifecycle Summary

Receipt lifecycle:

```text
Submitted
  -> Challenged
  -> Finalized

Challenged
  -> Finalized
  -> Rejected
  -> Slashed
```

Closable receipt states:

- `Finalized`
- `Rejected`
- `Slashed`

Challenge lifecycle:

```text
Open
  -> Accepted
  -> Rejected
```

Closable challenge states:

- `Accepted`
- `Rejected`
- `Expired`

## Events

Event definitions live in [events.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/events.rs).

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

## Testing

Current integration coverage in
[tests/clawfarm-attestation.ts](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/tests/clawfarm-attestation.ts):

- config initialization
- signer upsert
- missing `ed25519` pre-instruction rejection
- successful receipt submission
- unchallenged receipt finalization and close
- challenged receipt resolution and close
