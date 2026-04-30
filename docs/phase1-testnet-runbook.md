# Phase 1 Testnet Runbook

This runbook describes how to deploy and operate the current Phase 1 compact
receipt contract on Solana devnet.

## Prerequisites

- Anchor `0.32.1`
- Solana CLI `3.1.12`
- Yarn `1.x`
- funded admin keypair
- separate funded Test USDC operator keypair
- devnet wallets prepared for provider, payer, and challenger flows

## 1. Build and deploy both programs

```bash
yarn install
anchor build
solana program deploy target/deploy/clawfarm_masterpool.so --program-id target/deploy/clawfarm_masterpool-keypair.json --upgrade-authority <admin-keypair.json> --use-rpc --url "${SOLANA_RPC_URL}" --max-sign-attempts 1000
solana program deploy target/deploy/clawfarm_attestation.so --program-id target/deploy/clawfarm_attestation-keypair.json --upgrade-authority <admin-keypair.json> --use-rpc --url "${SOLANA_RPC_URL}" --max-sign-attempts 1000
```

Important:

- after the compact receipt ABI change, devnet must be running the current
  binaries before any compact smoketest can succeed
- a pre-compact attestation deployment will fail on `upsert_provider_signer`
  because the old ABI still expects a leading provider-code string argument

## 2. Bootstrap the fixed token pair

```bash
yarn phase1:bootstrap:testnet   --cluster devnet   --rpc-url "${SOLANA_RPC_URL}"   --admin-keypair <admin-keypair.json>   --test-usdc-operator-keypair <test-usdc-operator-keypair.json>   --masterpool-program-id AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux   --attestation-program-id 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2   --out deployments/devnet-phase1.json
```

This step creates:

- a fixed `CLAW` mint with `6` decimals
- a fixed `Test USDC` mint with `6` decimals
- protocol vault accounts
- a deployment record with all important addresses

## 3. Verify the deployment record

Confirm the generated deployment JSON includes at least:

- `clawMint`
- `testUsdcMint`
- `poolAuthority`
- `masterpoolConfig`
- `attestationConfig`
- `rewardVault`
- `challengeBondVault`
- `treasuryUsdcVault`
- `providerStakeUsdcVault`
- `providerPendingUsdcVault`
- `adminAuthority`
- `testUsdcOperator`

Also verify on chain:

- `CLAW` mint authority is revoked after genesis minting
- `CLAW` freeze authority is revoked
- reward vault holds the full genesis allocation
- `Test USDC` mint authority remains the external operator wallet
- masterpool config binds the exact `clawMint` and `testUsdcMint`

## 4. Fund test wallets

Mint test settlement funds with the external operator wallet:

```bash
yarn phase1:mint:test-usdc   --deployment deployments/devnet-phase1.json   --operator-keypair <test-usdc-operator-keypair.json>   --recipient <RECIPIENT_PUBKEY>   --amount 250
```

Recommended preparation:

- provider wallet funded with SOL and enough Test USDC for stake
- payer wallet funded with SOL and enough Test USDC for receipt charges
- challenger wallet funded with SOL and enough `CLAW` for challenge bond tests

## 5. Compact receipt integration notes

The gateway now submits compact receipts only.

On-chain receipt args are:

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

Operational rules:

- raw request nonce stays off chain; chain only stores `request_nonce_hash`
- rich metadata stays off chain; chain only stores `metadata_hash`
- `receipt_hash` is the primary external receipt id
- provider signer PDA is `("provider_signer", provider_wallet, signer)`
- receipt PDA is `("receipt", request_nonce_hash)`
- settlement mint is config-bound; arbitrary mints are rejected

## 6. Run the devnet smoketest

```bash
yarn phase1:smoketest:devnet   --deployment deployments/devnet-phase1.json   --config ./tmp/phase1-smoketest.devnet.json   --out ./tmp/phase1-smoketest-report.json
```

Expected coverage:

- provider signer upsert
- provider registration or active-state reuse
- negative provider registration with `InvalidUsdcMint`
- negative receipt path with config-bound `InvalidUsdcMint`
- positive compact receipt submission
- receipt lookup by `receipt_hash`

Expected successful report shape:

```json
{
  "status": "ok",
  "steps": {
    "invalidUsdcMint": {
      "matchedError": "InvalidUsdcMint"
    },
    "receiptInvalidUsdcMint": {
      "matchedError": "InvalidUsdcMint"
    },
    "receiptSubmission": {
      "receiptHashHex": "0x..."
    }
  }
}
```

## 7. Operational notes

- do not reuse the admin keypair as the Test USDC operator keypair
- keep raw request nonces and full metadata in gateway storage for support and
  challenge review
- use `receipt_hash` as the user-facing on-chain receipt id
- if devnet still runs the old string-based attestation deployment, redeploy the
  current programs before running the compact smoketest

## Devnet Faucet

The faucet is a devnet/testnet convenience feature in `clawfarm-masterpool`. It is disabled by default and must not be enabled on mainnet.

Initialize and enable it on devnet:

```bash
yarn phase1:faucet:configure --deployment deployments/devnet-phase1.json --admin-keypair <admin.json> --initialize
yarn phase1:faucet:configure --deployment deployments/devnet-phase1.json --admin-keypair <admin.json> --enable
```

Fund `CLAW` by moving existing genesis supply from `reward_vault` into `faucet_claw_vault`:

```bash
yarn phase1:faucet:fund --deployment deployments/devnet-phase1.json --admin-keypair <admin.json> --token claw --amount 1000
```

Fund `Test USDC` with the recorded operator wallet:

```bash
yarn phase1:faucet:fund --deployment deployments/devnet-phase1.json --funding-keypair <test-usdc-operator.json> --token usdc --amount 1000
```

Check faucet state, `reward_vault`, and faucet vault balances:

```bash
yarn phase1:faucet:status --deployment deployments/devnet-phase1.json
```

Claim faucet tokens with separate recipient and fee-payer identities:

```bash
yarn phase1:faucet:claim --deployment deployments/devnet-phase1.json --user-public-key <recipient-wallet-pubkey> --fee-payer-keypair <server-or-user-fee-payer.json> --claw-amount 10 --usdc-amount 10
```

Faucet claim identities are intentionally separate:

- `recipient` / `userPublicKey`: wallet public key that receives test tokens in its associated token accounts; it does not sign.
- `feePayer`: keypair that signs the transaction and pays devnet SOL fees plus any ATA rent; this can be a server wallet or the same wallet as the recipient.
- `sourceAuthority` / `tokenAuthority`: the faucet token-source authority; in this program it is the existing `pool_authority` PDA that controls `faucet_claw_vault` and `faucet_usdc_vault`.

Initial limits use base units on chain and six-decimal UI amounts in scripts:

- per claim: `10 CLAW` and `10 Test USDC`
- per wallet per UTC day: `50 CLAW` and `50 Test USDC`
- global per UTC day: `50,000 CLAW` and `50,000 Test USDC`

Mainnet preflight must pass only when the faucet config is absent or present with `enabled == false`.
