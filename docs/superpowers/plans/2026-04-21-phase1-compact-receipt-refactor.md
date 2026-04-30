# Phase 1 Compact Receipt Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current string-heavy Phase 1 receipt ABI with a fixed-size compact receipt contract that is safe for production-sized metadata, while keeping `receipt_hash` as the primary query and challenge identifier.

**Architecture:** Move all variable-length business metadata off chain behind `metadata_hash`, keep replay protection on `request_nonce_hash`, and make the on-chain receipt hash rebuild from fixed-size fields plus config-derived identities. Re-key provider signer registration by `(provider_wallet, signer)`, persist the signer pubkey in the signer account, and remove `charge_mint` from the attestation-to-masterpool settlement path.

**Tech Stack:** Anchor 0.32.1, Rust, Solana ed25519 precompile, TypeScript, Mocha/Chai, tsx, Solana devnet smoke script

---

Related spec: `docs/superpowers/specs/2026-04-21-phase1-compact-receipt-gateway-design.md`

## File Map

- Modify: `programs/clawfarm-attestation/src/state/types.rs`
  - Replace `SubmitReceiptArgs` with the compact fixed-size payload.
- Modify: `programs/clawfarm-attestation/src/state/accounts.rs`
  - Persist the signer pubkey inside `ProviderSigner`; keep `Receipt` minimal.
- Modify: `programs/clawfarm-attestation/src/utils.rs`
  - Remove string-validation and CBOR payload builders; add compact hash builders.
- Modify: `programs/clawfarm-attestation/src/instructions/admin.rs`
  - Re-key provider signer registration by `(provider_wallet, signer)`.
- Modify: `programs/clawfarm-attestation/src/instructions/receipt.rs`
  - Rebuild `receipt_hash` from fixed-size fields plus runtime accounts; update receipt PDA seeds.
- Modify: `programs/clawfarm-attestation/src/events.rs`
  - Emit hashes only; remove raw strings from events.
- Modify: `programs/clawfarm-attestation/src/lib.rs`
  - Update public instruction signatures and exported account builders.
- Modify: `programs/clawfarm-attestation/src/tests.rs`
  - Add unit coverage for compact args, provider signer seeds, and hash vectors.
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
  - Remove `charge_mint` from `RecordMiningFromReceiptArgs` and rely on config-bound mint checks.
- Modify: `programs/clawfarm-masterpool/src/instructions/mod.rs`
  - Re-export the trimmed receipt args type.
- Modify: `programs/clawfarm-masterpool/src/lib.rs`
  - Update the CPI entrypoint signature.
- Create: `scripts/phase1/compact-receipt.ts`
  - Centralize TypeScript hashing, compact arg building, and `receipt_hash` lookup helpers.
- Modify: `tests/phase1-integration.ts`
  - Switch integration helpers and tests to the compact contract.
- Modify: `scripts/phase1/devnet-smoketest.ts`
  - Submit compact receipts and look them up by `receipt_hash`.
- Modify: `scripts/phase1/devnet-smoketest.example.json`
  - Keep rich off-chain metadata fields, but document that only hashes go on chain.
- Modify: `tests/phase1-devnet-smoketest-script.ts`
  - Update CLI expectations for hash-based submission and lookup.
- Modify: `docs/clawfarm-attestation-phase1-abi.md`
  - Replace the old ABI section with the compact receipt contract.
- Modify: `docs/clawfarm-attestation-phase1-interface-design.md`
  - Update the formal interface description and event model.
- Modify: `docs/phase1-testnet-runbook.md`
  - Update runbook steps and smoke-test expectations.
- Modify: `programs/clawfarm-attestation/README.md`
  - Document compact hashing and wallet-keyed signer registration.
- Modify: `programs/clawfarm-masterpool/README.md`
  - Document the `charge_mint` removal and config-bound USDC path.

### Task 1: Replace the receipt payload with compact fixed-size hashes

**Files:**
- Modify: `programs/clawfarm-attestation/src/state/types.rs`
- Modify: `programs/clawfarm-attestation/src/utils.rs`
- Modify: `programs/clawfarm-attestation/src/tests.rs`

- [ ] **Step 1: Write the failing unit tests for compact args and compact hash preimage**

Add a Rust test module that proves the new args encode to a deterministic `120` bytes and that the compact hash changes when any fixed-size input changes.

```rust
#[test]
fn compact_submit_receipt_args_are_fixed_width() {
    let args = sample_submit_receipt_args_v2();
    let encoded = args.try_to_vec().unwrap();

    assert_eq!(encoded.len(), 120);
}

#[test]
fn compact_receipt_hash_changes_when_charge_changes() {
    let mut inputs = sample_compact_hash_inputs();
    let left = build_compact_receipt_hash(&inputs);
    inputs.charge_atomic += 1;
    let right = build_compact_receipt_hash(&inputs);

    assert_ne!(left, right);
}

fn sample_submit_receipt_args_v2() -> SubmitReceiptArgs {
    SubmitReceiptArgs {
        request_nonce_hash: [1; 32],
        metadata_hash: [2; 32],
        prompt_tokens: 123,
        completion_tokens: 456,
        charge_atomic: 10_000_000,
        receipt_hash: [3; 32],
    }
}

fn sample_compact_hash_inputs() -> CompactReceiptHashInputs {
    CompactReceiptHashInputs {
        request_nonce_hash: [1; 32],
        metadata_hash: [2; 32],
        provider_wallet: Pubkey::new_from_array([3; 32]),
        payer_user: Pubkey::new_from_array([4; 32]),
        usdc_mint: Pubkey::new_from_array([5; 32]),
        prompt_tokens: 123,
        completion_tokens: 456,
        charge_atomic: 10_000_000,
    }
}
```

- [ ] **Step 2: Run the new unit tests and verify they fail before implementation**

Run:

```bash
cargo test -q compact_submit_receipt_args_are_fixed_width
cargo test -q compact_receipt_hash_changes_when_charge_changes
```

Expected:

```text
error[E0422]: cannot find struct, variant or union type `CompactReceiptHashInputs`
...
thread 'compact_submit_receipt_args_are_fixed_width' panicked
```

- [ ] **Step 3: Implement the compact args type and compact receipt hash builder**

Replace the old string-heavy payload and CBOR builder with a compact fixed-size type and a deterministic preimage builder.

```rust
// programs/clawfarm-attestation/src/state/types.rs
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

```rust
// programs/clawfarm-attestation/src/utils.rs
pub(crate) const COMPACT_RECEIPT_DOMAIN_SEPARATOR: &[u8] = b"clawfarm:receipt:v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactReceiptHashInputs {
    pub request_nonce_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub provider_wallet: Pubkey,
    pub payer_user: Pubkey,
    pub usdc_mint: Pubkey,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub charge_atomic: u64,
}

pub(crate) fn validate_submit_receipt_args(args: &SubmitReceiptArgs) -> Result<()> {
    args.prompt_tokens
        .checked_add(args.completion_tokens)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
    Ok(())
}

pub(crate) fn build_compact_receipt_hash(inputs: &CompactReceiptHashInputs) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(
        COMPACT_RECEIPT_DOMAIN_SEPARATOR.len() + (32 * 5) + (8 * 3),
    );
    preimage.extend_from_slice(COMPACT_RECEIPT_DOMAIN_SEPARATOR);
    preimage.extend_from_slice(&inputs.request_nonce_hash);
    preimage.extend_from_slice(&inputs.metadata_hash);
    preimage.extend_from_slice(inputs.provider_wallet.as_ref());
    preimage.extend_from_slice(inputs.payer_user.as_ref());
    preimage.extend_from_slice(inputs.usdc_mint.as_ref());
    preimage.extend_from_slice(&inputs.prompt_tokens.to_le_bytes());
    preimage.extend_from_slice(&inputs.completion_tokens.to_le_bytes());
    preimage.extend_from_slice(&inputs.charge_atomic.to_le_bytes());
    hash(&preimage).to_bytes()
}
```

- [ ] **Step 4: Re-run the Rust unit tests and verify they pass**

Run:

```bash
cargo test -q compact_submit_receipt_args_are_fixed_width
cargo test -q compact_receipt_hash_changes_when_charge_changes
```

Expected:

```text
test result: ok. 1 passed; 0 failed
...
test result: ok. 1 passed; 0 failed
```

- [ ] **Step 5: Commit the compact payload foundation**

```bash
git add programs/clawfarm-attestation/src/state/types.rs \
  programs/clawfarm-attestation/src/utils.rs \
  programs/clawfarm-attestation/src/tests.rs
git commit -m "refactor: compact attestation receipt payload"
```

### Task 2: Re-key provider signer registration by provider wallet and signer

**Files:**
- Modify: `programs/clawfarm-attestation/src/state/accounts.rs`
- Modify: `programs/clawfarm-attestation/src/instructions/admin.rs`
- Modify: `programs/clawfarm-attestation/src/events.rs`
- Modify: `programs/clawfarm-attestation/src/lib.rs`
- Modify: `programs/clawfarm-attestation/src/tests.rs`
- Modify: `tests/phase1-integration.ts`

- [ ] **Step 1: Write the failing tests for wallet-keyed signer PDAs**

Add one Rust account-space regression for the new stored signer pubkey plus one
integration helper assertion that derives the signer PDA from `(provider_wallet,
signer)` instead of `provider_code`.

```rust
#[test]
fn provider_signer_space_includes_signer_pubkey() {
    assert_eq!(ProviderSigner::INIT_SPACE, 32 + 32 + 1 + 1 + 8 + 8);
}
```

```ts
const providerSignerPda = PublicKey.findProgramAddressSync(
  [
    Buffer.from("provider_signer"),
    providerWallet.publicKey.toBuffer(),
    providerSigner.publicKey.toBuffer(),
  ],
  attestation.programId
)[0];
expect(providerSignerPda).to.be.instanceOf(PublicKey);
```

- [ ] **Step 2: Run the signer-registry tests and verify the integration helper path fails first**

Run:

```bash
cargo test -q provider_signer_space_includes_signer_pubkey
yarn test --grep "Phase 1 core economics"
```

Expected:

```text
thread 'provider_signer_space_includes_signer_pubkey' panicked
...
AnchorError: ConstraintSeeds
```

- [ ] **Step 3: Persist the signer pubkey and switch the PDA seeds in admin instructions**

Update the account shape, instruction signatures, and events so the signer registry is wallet-native.

```rust
// programs/clawfarm-attestation/src/state/accounts.rs
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

```rust
// programs/clawfarm-attestation/src/instructions/admin.rs
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_wallet: Pubkey,
    signer: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()> {
    let provider_signer = &mut ctx.accounts.provider_signer;
    provider_signer.provider_wallet = provider_wallet;
    provider_signer.signer = signer;
    provider_signer.attester_type_mask = attester_type_mask;
    provider_signer.status = SignerStatus::Active as u8;
    provider_signer.valid_from = valid_from;
    provider_signer.valid_until = valid_until;
    Ok(())
}

#[account(
    init_if_needed,
    payer = authority,
    space = 8 + ProviderSigner::INIT_SPACE,
    seeds = [
        PROVIDER_SIGNER_SEED,
        provider_wallet.as_ref(),
        signer.as_ref(),
    ],
    bump
)]
pub provider_signer: Account<'info, ProviderSigner>;
```

```rust
// programs/clawfarm-attestation/src/events.rs
#[event]
pub struct ProviderSignerUpserted {
    pub signer: Pubkey,
    pub provider_wallet: Pubkey,
    pub attester_type_mask: u8,
}

#[event]
pub struct ProviderSignerRevoked {
    pub signer: Pubkey,
    pub provider_wallet: Pubkey,
}
```

- [ ] **Step 4: Re-run the signer-registry unit and integration tests**

Run:

```bash
cargo test -q provider_signer_space_includes_signer_pubkey
yarn test --grep "Phase 1 core economics"
```

Expected:

```text
test result: ok. 1 passed; 0 failed
...
Phase 1 core economics
  ...
  passing
```

- [ ] **Step 5: Commit the signer-registry refactor**

```bash
git add programs/clawfarm-attestation/src/state/accounts.rs \
  programs/clawfarm-attestation/src/instructions/admin.rs \
  programs/clawfarm-attestation/src/events.rs \
  programs/clawfarm-attestation/src/lib.rs \
  programs/clawfarm-attestation/src/tests.rs \
  tests/phase1-integration.ts
git commit -m "refactor: wallet-key provider signer registry"
```

### Task 3: Compact the submit flow and remove `charge_mint` from settlement CPIs

**Files:**
- Modify: `programs/clawfarm-attestation/src/instructions/receipt.rs`
- Modify: `programs/clawfarm-attestation/src/events.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- Modify: `programs/clawfarm-masterpool/src/instructions/mod.rs`
- Modify: `programs/clawfarm-masterpool/src/lib.rs`
- Modify: `tests/phase1-integration.ts`

- [ ] **Step 1: Write the failing integration tests for long metadata and rogue mint accounts**

Add one integration case that uses very long `proof_id` and `model` strings in off-chain metadata but still expects submit success, plus one negative case that passes a rogue `usdc_mint` account and expects the config-bound mint check to fail.

```ts
it("submits with long off-chain metadata because only hashes go on chain", async () => {
  const receipt = await submitCompactReceipt("phase1-long-metadata", {
    metadata: {
      proofId: "proof-" + "x".repeat(512),
      providerCode: "gateway/" + "y".repeat(256),
      model: "model-" + "z".repeat(512),
    },
  });

  expect(receipt.signature).to.match(/[1-9A-HJ-NP-Za-km-z]{32,}/);
});

it("rejects rogue usdc mint accounts after charge_mint removal", async () => {
  await expectAnchorError(
    submitCompactReceipt("phase1-rogue-mint", {
      usdcMintOverride: rogueUsdcMint,
    }),
    "InvalidUsdcMint"
  );
});
```

- [ ] **Step 2: Run the integration suite and verify the new cases fail before implementation**

Run:

```bash
yarn test --grep "long off-chain metadata|rogue usdc mint"
```

Expected:

```text
expected error containing InvalidUsdcMint
...
Transaction too large
```

- [ ] **Step 3: Rebuild `submit_receipt` from compact inputs and trim the masterpool CPI args**

Use `provider_signer.signer`, `provider_signer.provider_wallet`, `payer_user.key()`, and the config-bound `usdc_mint` account to rebuild the compact hash and settle without `charge_mint`.

```rust
// programs/clawfarm-attestation/src/instructions/receipt.rs
let provider_signer = &ctx.accounts.provider_signer;
let compact_hash = build_compact_receipt_hash(&CompactReceiptHashInputs {
    request_nonce_hash: args.request_nonce_hash,
    metadata_hash: args.metadata_hash,
    provider_wallet: provider_signer.provider_wallet,
    payer_user: ctx.accounts.payer_user.key(),
    usdc_mint: ctx.accounts.usdc_mint.key(),
    prompt_tokens: args.prompt_tokens,
    completion_tokens: args.completion_tokens,
    charge_atomic: args.charge_atomic,
});
require!(compact_hash == args.receipt_hash, ErrorCode::ReceiptHashMismatch);

verify_preceding_ed25519_instruction(
    &ctx.accounts.instructions_sysvar.to_account_info(),
    &provider_signer.signer,
    &args.receipt_hash,
)?;

receipt.receipt_hash = args.receipt_hash;
receipt.signer = provider_signer.signer;
receipt.payer_user = ctx.accounts.payer_user.key();
receipt.provider_wallet = provider_signer.provider_wallet;

clawfarm_masterpool::cpi::record_mining_from_receipt(
    cpi_ctx,
    RecordMiningFromReceiptArgs {
        total_usdc_paid: args.charge_atomic,
    },
)?;

emit!(ReceiptSubmitted {
    receipt: receipt_key,
    request_nonce_hash: args.request_nonce_hash,
    metadata_hash: args.metadata_hash,
    signer: provider_signer.signer,
    receipt_hash: args.receipt_hash,
    challenge_deadline: receipt.challenge_deadline,
});
```

```rust
// receipt PDA seed
#[account(
    init,
    payer = authority,
    space = 8 + Receipt::INIT_SPACE,
    seeds = [RECEIPT_SEED, &args.request_nonce_hash],
    bump
)]
pub receipt: Box<Account<'info, Receipt>>;
```

```rust
// programs/clawfarm-masterpool/src/instructions/receipt.rs
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordMiningFromReceiptArgs {
    pub total_usdc_paid: u64,
}

pub fn record_mining_from_receipt(
    ctx: Context<RecordMiningFromReceipt>,
    args: RecordMiningFromReceiptArgs,
) -> Result<()> {
    require_attestation_caller(config, &ctx.accounts.attestation_config.to_account_info())?;
    require!(!config.pause_receipt_recording, ErrorCode::ReceiptRecordingPaused);
    require!(args.total_usdc_paid > 0, ErrorCode::InvalidPositiveAmount);
    validate_supported_receipt_charge(args.total_usdc_paid)?;
    require_token_owner(&ctx.accounts.payer_usdc_token, &ctx.accounts.payer_user.key())?;
    require_token_mint(&ctx.accounts.payer_usdc_token, &config.usdc_mint)?;
    ...
}
```

- [ ] **Step 4: Re-run the targeted integration tests and full local suite**

Run:

```bash
yarn test --grep "long off-chain metadata|rogue usdc mint"
yarn test
cargo test -q
```

Expected:

```text
2 passing
...
Phase 1 core economics
  ...
  passing
...
test result: ok.
```

- [ ] **Step 5: Commit the compact submit-flow refactor**

```bash
git add programs/clawfarm-attestation/src/instructions/receipt.rs \
  programs/clawfarm-attestation/src/events.rs \
  programs/clawfarm-masterpool/src/instructions/receipt.rs \
  programs/clawfarm-masterpool/src/instructions/mod.rs \
  programs/clawfarm-masterpool/src/lib.rs \
  tests/phase1-integration.ts
git commit -m "refactor: compact receipt settlement flow"
```

### Task 4: Share compact receipt helpers across tests and the devnet smoke script

**Files:**
- Create: `scripts/phase1/compact-receipt.ts`
- Modify: `tests/phase1-integration.ts`
- Modify: `scripts/phase1/devnet-smoketest.ts`
- Modify: `scripts/phase1/devnet-smoketest.example.json`
- Modify: `tests/phase1-devnet-smoketest-script.ts`

- [ ] **Step 1: Write the failing script-level tests for helper parity and hash lookup**

Add a script test that proves the shared helper produces the same `receipt_hash` for the same logical receipt, and that the helper can locate a submitted receipt by `receipt_hash` using a memcmp query.

```ts
it("builds deterministic compact receipt hashes", async () => {
  const left = buildCompactReceipt({
    rawRequestNonce: "nonce-1",
    metadata: { proofId: "p", providerCode: "gateway", model: "m" },
    providerWallet: providerWallet.publicKey,
    payerUser: payerUser.publicKey,
    usdcMint,
    promptTokens: 123,
    completionTokens: 456,
    chargeAtomic: 10_000_000,
  });
  const right = buildCompactReceipt({
    rawRequestNonce: "nonce-1",
    metadata: { proofId: "p", providerCode: "gateway", model: "m" },
    providerWallet: providerWallet.publicKey,
    payerUser: payerUser.publicKey,
    usdcMint,
    promptTokens: 123,
    completionTokens: 456,
    chargeAtomic: 10_000_000,
  });

  expect(left.receiptHashHex).to.equal(right.receiptHashHex);
});

it("finds a receipt by receipt_hash", async () => {
  const submitted = await submitCompactReceipt("phase1-hash-lookup");
  const found = await findReceiptByHash(
    provider.connection,
    attestation.programId,
    submitted.receiptHash
  );

  expect(found?.toBase58()).to.equal(submitted.receiptPda.toBase58());
});
```

- [ ] **Step 2: Run the script tests and verify they fail before the shared helper exists**

Run:

```bash
npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-devnet-smoketest-script.ts
```

Expected:

```text
ReferenceError: buildCompactReceipt is not defined
...
TypeError: findReceiptByHash is not a function
```

- [ ] **Step 3: Create the shared helper and switch the smoke script to compact args**

Centralize request nonce hashing, metadata hashing, compact receipt hashing, human-readable hash formatting, and `receipt_hash` lookup in one reusable file.

```ts
// scripts/phase1/compact-receipt.ts
import { BN } from "@coral-xyz/anchor";
import crypto from "crypto";
import bs58 from "bs58";
import { Connection, PublicKey } from "@solana/web3.js";

export type ReceiptMetadata = {
  proofId: string;
  providerCode: string;
  model: string;
  providerRequestId?: string;
  issuedAt?: number;
  expiresAt?: number;
};

export function buildCompactReceipt(args: {
  rawRequestNonce: string;
  metadata: ReceiptMetadata;
  providerWallet: PublicKey;
  payerUser: PublicKey;
  usdcMint: PublicKey;
  promptTokens: number;
  completionTokens: number;
  chargeAtomic: number;
}) {
  const requestNonceHash = sha256(Buffer.from(args.rawRequestNonce, "utf8"));
  const metadataHash = sha256(
    Buffer.from(stableMetadataJson(args.metadata), "utf8")
  );
  const receiptHash = sha256(
    Buffer.concat([
      Buffer.from("clawfarm:receipt:v2", "utf8"),
      requestNonceHash,
      metadataHash,
      args.providerWallet.toBuffer(),
      args.payerUser.toBuffer(),
      args.usdcMint.toBuffer(),
      u64Le(args.promptTokens),
      u64Le(args.completionTokens),
      u64Le(args.chargeAtomic),
    ])
  );

  return {
    requestNonceHash,
    metadataHash,
    receiptHash,
    receiptHashHex: `0x${receiptHash.toString("hex")}`,
    submitArgs: {
      requestNonceHash: Array.from(requestNonceHash),
      metadataHash: Array.from(metadataHash),
      promptTokens: new BN(args.promptTokens),
      completionTokens: new BN(args.completionTokens),
      chargeAtomic: new BN(args.chargeAtomic),
      receiptHash: Array.from(receiptHash),
    },
  };
}

function stableMetadataJson(metadata: ReceiptMetadata): string {
  const value = {
    schema: "clawfarm-receipt-metadata/v2",
    proof_id: metadata.proofId,
    provider_code: metadata.providerCode,
    model: metadata.model,
    ...(metadata.providerRequestId
      ? { provider_request_id: metadata.providerRequestId }
      : {}),
    ...(metadata.issuedAt !== undefined ? { issued_at: metadata.issuedAt } : {}),
    ...(metadata.expiresAt !== undefined
      ? { expires_at: metadata.expiresAt }
      : {}),
  };
  const orderedEntries = Object.entries(value).sort(([left], [right]) =>
    left.localeCompare(right)
  );
  return JSON.stringify(Object.fromEntries(orderedEntries));
}

function sha256(data: Buffer): Buffer {
  return crypto.createHash("sha256").update(data).digest();
}

function u64Le(value: number): Buffer {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value));
  return out;
}

export async function findReceiptByHash(
  connection: Connection,
  attestationProgramId: PublicKey,
  receiptHash: Buffer
): Promise<PublicKey | null> {
  const matches = await connection.getProgramAccounts(attestationProgramId, {
    filters: [{ memcmp: { offset: 8, bytes: bs58.encode(receiptHash) } }],
  });
  return matches[0]?.pubkey ?? null;
}
```

- [ ] **Step 4: Re-run the script suite and the devnet smoke script**

Run:

```bash
npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-devnet-smoketest-script.ts

yarn phase1:smoketest:devnet \
  --deployment deployments/devnet-phase1.json \
  --config ./tmp/phase1-smoketest.devnet.json \
  --out ./tmp/phase1-smoketest-report.json
```

Expected:

```text
  Phase 1 devnet smoketest script
    ...
    passing
```

```json
{
  "status": "ok",
  "steps": {
    "receiptSubmission": {
      "receiptHash": "0x..."
    }
  }
}
```

- [ ] **Step 5: Commit the shared helper and smoke-test updates**

```bash
git add scripts/phase1/compact-receipt.ts \
  tests/phase1-integration.ts \
  scripts/phase1/devnet-smoketest.ts \
  scripts/phase1/devnet-smoketest.example.json \
  tests/phase1-devnet-smoketest-script.ts
git commit -m "test: switch phase1 scripts to compact receipts"
```

### Task 5: Update formal docs and run the full regression pack

**Files:**
- Modify: `docs/clawfarm-attestation-phase1-abi.md`
- Modify: `docs/clawfarm-attestation-phase1-interface-design.md`
- Modify: `docs/phase1-testnet-runbook.md`
- Modify: `programs/clawfarm-attestation/README.md`
- Modify: `programs/clawfarm-masterpool/README.md`

- [ ] **Step 1: Update the docs to match the compact receipt contract exactly**

Apply the same final shapes from the spec to the ABI, interface, README, and runbook docs.

Document the following contract details verbatim:

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

Provider signer PDA:

```text
["provider_signer", provider_wallet, signer]
```

Receipt lookup:

- primary external ID: `receipt_hash`
- chain query: memcmp on `Receipt.receipt_hash` at offset `8`

- [ ] **Step 2: Grep the repo for stale V1 receipt fields and remove the remaining references**

Run:

```bash
rg -n "proof_id: String|request_nonce: String|provider_code: String|charge_mint: Pubkey|total_tokens|http_status|latency_ms" \
  docs/clawfarm-attestation-phase1-abi.md \
  docs/clawfarm-attestation-phase1-interface-design.md \
  docs/phase1-testnet-runbook.md \
  programs/clawfarm-attestation/README.md \
  programs/clawfarm-masterpool/README.md
```

Expected:

```text
(no output)
```

The historical migration note can stay in the new gateway spec because it is not
part of the runtime docs.

- [ ] **Step 3: Run the full local regression suite**

Run:

```bash
cargo test -q
yarn test
npx ts-mocha -p ./tsconfig.json -t 1000000 \
  tests/phase1-script-helpers.ts \
  tests/phase1-bootstrap-script.ts \
  tests/phase1-test-usdc-script.ts \
  tests/phase1-devnet-smoketest-script.ts
```

Expected:

```text
test result: ok.
...
Phase 1 core economics
  ...
  passing
...
  Phase 1 devnet smoketest script
    ...
    passing
```

- [ ] **Step 4: Re-run the devnet smoke test against the existing deployment**

Run:

```bash
yarn phase1:smoketest:devnet \
  --deployment deployments/devnet-phase1.json \
  --config ./tmp/phase1-smoketest.devnet.json \
  --out ./tmp/phase1-smoketest-report.json
```

Expected:

```json
{
  "status": "ok",
  "steps": {
    "invalidUsdcMint": {
      "matchedError": "InvalidUsdcMint"
    },
    "receiptSubmission": {
      "receiptHash": "0x..."
    }
  }
}
```

- [ ] **Step 5: Commit the docs and final verification state**

```bash
git add docs/clawfarm-attestation-phase1-abi.md \
  docs/clawfarm-attestation-phase1-interface-design.md \
  docs/phase1-testnet-runbook.md \
  programs/clawfarm-attestation/README.md \
  programs/clawfarm-masterpool/README.md
git commit -m "docs: publish compact receipt contract"
```
