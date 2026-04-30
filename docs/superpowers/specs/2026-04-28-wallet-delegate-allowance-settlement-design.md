# Wallet Delegate Allowance Settlement Design

Status: Implemented in `clawfarm-masterpool`, merged to `main`, and deployed on devnet
Date: 2026-04-29
Audience: clawfarm-masterpool and AIRouter engineers
Scope: Update the devnet receipt settlement path so browser wallet requests can pay from the browser wallet USDC account without requiring the browser wallet to sign the server-side receipt transaction.

Implementation update: contract changes landed on 2026-04-28 in merge commit `d2718f7`. The merged implementation keeps `payer_user` in receipt identity, adds `fee_payer` and `payment_delegate`, validates SPL Token delegate allowance in masterpool, and verifies the full local suite with `yarn test` passing 11 integration tests. `target/idl` and `target/types` are generated locally by `anchor build` but are not tracked in this repository checkout.

## Problem

The current `submit_receipt` path is not compatible with the wallet-native browser Gateway flow.

AIRouter correctly builds receipts with:

- `payer_user = browser wallet`
- `payer_usdc_token = browser wallet USDC ATA`
- `provider_wallet = settlement profile provider wallet`

However, the current on-chain ABI also requires `payer_user` to be a transaction signer. The masterpool CPI uses the same `payer_user` signer as the SPL Token transfer authority and as the rent payer for `user_reward_account` and `receipt_settlement` creation.

This works only when the server has the user's wallet keypair. It fails for real site traffic because the Gateway only receives a signed HTTP payment intent. The Gateway must not hold the browser wallet private key.

## Target Outcome

A browser wallet can authorize ClawFarm spending once by approving a delegate allowance on its USDC token account. Later, AIRouter can submit a settled receipt server-side:

1. `payer_user` remains the browser wallet and is recorded on chain.
2. `payer_usdc_token` remains the browser wallet USDC ATA.
3. `fee_payer` pays SOL rent/transaction fees and signs the transaction.
4. `payment_delegate` signs the SPL Token transfer as the approved delegate.
5. `payer_user` no longer signs `submit_receipt`.
6. The transaction still verifies the Gateway/provider receipt signature with the existing Ed25519 instruction.

For the devnet MVP, AIRouter can use the same keypair for `fee_payer` and `payment_delegate` (`PayerKeypairFile`). The ABI should keep the roles separate so production can split them later.

The local contract tests use the same keypair for `fee_payer` and `payment_delegate` by default. This matches the devnet MVP and keeps the current legacy transaction with Ed25519 receipt verification under the Solana transaction size limit.

## Contract Changes

### Attestation Program: `submit_receipt`

Primary files:

- `programs/clawfarm-attestation/src/instructions/receipt.rs`
- `programs/clawfarm-attestation/src/state/accounts.rs`
- generated IDL under `target/idl/clawfarm_attestation.json` after build

Update `SubmitReceipt` accounts:

```rust
pub struct SubmitReceipt<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    // existing config/provider_signer/receipt accounts unchanged

    /// CHECK: business payer wallet; not a signer in delegate mode
    #[account(mut)]
    pub payer_user: UncheckedAccount<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub payment_delegate: Signer<'info>,

    #[account(mut)]
    /// CHECK: validated by masterpool
    pub payer_usdc_token: UncheckedAccount<'info>,

    // existing masterpool accounts unchanged, plus payment_delegate/fee_payer
}
```

Changes inside `submit_receipt`:

- Keep `payer_user = ctx.accounts.payer_user.key()` in the receipt hash preimage.
- Keep `receipt.payer_user = ctx.accounts.payer_user.key()`.
- Keep the receipt PDA rent payer as `authority` unless a separate fee-payer migration is desired for that account.
- Pass both `fee_payer` and `payment_delegate` into `record_mining_from_receipt`.
- Do not require `payer_user.is_signer`.

### Masterpool Program: `record_mining_from_receipt`

Primary files:

- `programs/clawfarm-masterpool/src/instructions/receipt.rs`
- `programs/clawfarm-masterpool/src/errors.rs`
- generated IDL under `target/idl/clawfarm_masterpool.json` after build

Update `RecordMiningFromReceipt` accounts:

```rust
pub struct RecordMiningFromReceipt<'info> {
    // existing config and attestation_config unchanged

    /// CHECK: business payer wallet, validated as token owner
    pub payer_user: UncheckedAccount<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub payment_delegate: Signer<'info>,

    #[account(mut)]
    pub payer_usdc_token: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        space = REWARD_ACCOUNT_SPACE,
        seeds = [USER_REWARD_SEED, payer_user.key().as_ref()],
        bump,
    )]
    pub user_reward_account: Box<Account<'info, RewardAccount>>,

    #[account(
        init,
        payer = fee_payer,
        space = RECEIPT_SETTLEMENT_SPACE,
        seeds = [RECEIPT_SETTLEMENT_SEED, attestation_receipt.key().as_ref()],
        bump,
    )]
    pub receipt_settlement: Box<Account<'info, ReceiptSettlement>>,

    // remaining provider/reward/vault/mint/program accounts unchanged
}
```

Transfer logic changes:

```rust
require_token_owner(&ctx.accounts.payer_usdc_token, &ctx.accounts.payer_user.key())?;
require_token_mint(&ctx.accounts.payer_usdc_token, &config.usdc_mint)?;
require!(
    ctx.accounts.payer_usdc_token.delegate == COption::Some(ctx.accounts.payment_delegate.key()),
    ErrorCode::InvalidPaymentDelegate
);
require!(
    ctx.accounts.payer_usdc_token.delegated_amount >= args.total_usdc_paid,
    ErrorCode::InsufficientDelegatedAllowance
);
```

Use `payment_delegate` as the SPL Token transfer authority for both treasury and provider-pending transfers:

```rust
authority: ctx.accounts.payment_delegate.to_account_info(),
```

The SPL Token program will decrement the delegated allowance as transfers execute.

## New Errors

Add explicit errors to masterpool:

- `InvalidPaymentDelegate`: payer token account delegate is not the supplied `payment_delegate`.
- `InsufficientDelegatedAllowance`: delegated allowance is below `total_usdc_paid`.

The existing owner/mint/positive-amount/provider-status errors remain unchanged.

## AIRouter Alignment

AIRouter should treat `PayerKeypairFile` as the transaction fee payer and devnet payment delegate keypair:

- `payer_user` in the receipt remains the verified browser wallet.
- `payer_usdc_token` is the browser wallet ATA.
- transaction `feePayer` is `PayerKeypairFile`.
- `fee_payer` account is `PayerKeypairFile.publicKey`.
- `payment_delegate` account is `PayerKeypairFile.publicKey` for MVP.
- AIRouter must not attempt to sign as `payer_user`.

A later config can split these roles, for example `FeePayerKeypairFile` and `PaymentDelegateKeypairFile`, but this is not required for the devnet MVP.

AIRouter must use the deployed delegate-allowance IDL from `ContractRepoDir`. The helper sets the Solana transaction `feePayer` to `PayerKeypairFile`, signs with both `AuthorityKeypairFile` and `PayerKeypairFile`, and passes `fee_payer` plus `payment_delegate` accounts when the IDL exposes them. For the devnet MVP, both accounts map to `PayerKeypairFile.publicKey`.

## Frontend/Site Alignment

Before browser wallet calls that require payment, the site must ensure the user's USDC ATA has approved the configured payment delegate.

The site should:

1. Fetch or be configured with the Gateway payment delegate pubkey.
2. Build an SPL Token `ApproveChecked` transaction for the user's USDC ATA.
3. Let the browser wallet sign it.
4. Approve a bounded amount and show the remaining allowance to the user.
5. Re-approve when the allowance is too low.

This approval is separate from the HTTP payment intent. The HTTP payment intent remains the per-request max-charge authorization; the SPL delegate allowance is the on-chain spending capability.

## Security Requirements

- The per-request signed `max_charge_atomic` remains enforced by AIRouter before settlement.
- The on-chain delegated allowance must be checked by the Token Program and should be prechecked by Gateway when RPC is available.
- `payer_user` must remain part of the compact receipt hash preimage.
- `payer_user` must remain the owner of `payer_usdc_token`.
- `payment_delegate` should be revocable by the user through standard SPL Token revoke.
- The delegate allowance should be limited and re-approved by the user, not unlimited by default.

## Test Plan

Contract unit tests:

- `submit_receipt` succeeds when `payer_user` is not signer, `fee_payer` signs, and `payment_delegate` has enough delegated allowance.
- Receipt hash validation still binds the browser wallet `payer_user`.
- Masterpool rejects payer token accounts not owned by `payer_user`.
- Masterpool rejects wrong `payment_delegate`.
- Masterpool rejects insufficient delegated allowance.
- Rent for `user_reward_account` and `receipt_settlement` is paid by `fee_payer`.

Integration/devnet tests:

- User approves delegate for bounded USDC allowance.
- AIRouter submits a browser wallet receipt with `PayerKeypairFile` as fee payer/delegate and browser wallet as `payer_user`.
- User USDC balance decreases, treasury/provider pending vault balances increase, and receipt settlement records the browser wallet.
- Revoked or insufficient allowance produces a clear failure before provider response release.

## Rollout Plan

1. Implement and test the masterpool account/transfer-authority changes first.
2. Implement and test the attestation CPI account changes against the updated masterpool IDL.
3. Rebuild IDLs and deploy both programs together; do not mix the new AIRouter helper with an old attestation IDL for wallet-browser settlement tests.
4. Update AIRouter `ContractRepoDir` to the deployed checkout and restart `clawfarm-chain-worker`.
5. Have the site approve `PayerKeypairFile.publicKey` as the USDC delegate before sending paid wallet requests.
6. Run one end-to-end browser-wallet request and verify: browser wallet is stored as `payer_user`, `PayerKeypairFile` paid the transaction fee, delegated USDC allowance decreased, and the provider response is released only after settlement succeeds.

Contract implementation status:
- Steps 1-3 are complete in the contract repo, merged to `main`, and deployed on devnet.
- Steps 4-6 remain AIRouter/site rollout and end-to-end verification work.
