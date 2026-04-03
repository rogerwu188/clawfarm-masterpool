use anchor_lang::prelude::*;
use solana_sdk_ids::sysvar::instructions::ID as INSTRUCTIONS_SYSVAR_ID;
use solana_sha256_hasher::hash;

use crate::{
    constants::{CONFIG_SEED, PROVIDER_SIGNER_SEED, RECEIPT_SEED},
    error::ErrorCode,
    events::{ReceiptFinalized, ReceiptSubmitted},
    state::{Config, ProviderSigner, Receipt, ReceiptStatus, SignerStatus, SubmitReceiptArgs},
    utils::{
        attester_type_mask, build_phase1_canonical_cbor, provider_signer_seed, request_nonce_seed,
        validate_request_nonce, validate_submit_receipt_args, verify_preceding_ed25519_instruction,
    },
};

pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()> {
    validate_submit_receipt_args(&args)?;

    let now = Clock::get()?.unix_timestamp;
    let config = &mut ctx.accounts.config;
    require!(!config.is_paused, ErrorCode::ProgramPaused);

    let provider_signer = &ctx.accounts.provider_signer;
    require!(
        provider_signer.status == SignerStatus::Active as u8,
        ErrorCode::SignerInactive
    );
    require!(
        provider_signer.signer == args.signer,
        ErrorCode::SignerMismatch
    );
    require!(
        provider_signer.provider_code == args.provider,
        ErrorCode::ProviderMismatch
    );
    require!(
        provider_signer.attester_type_mask & attester_type_mask(args.attester_type) != 0,
        ErrorCode::SignerAttesterTypeMismatch
    );
    require!(
        now >= provider_signer.valid_from,
        ErrorCode::SignerNotYetValid
    );
    require!(
        provider_signer.valid_until == 0 || now <= provider_signer.valid_until,
        ErrorCode::SignerExpired
    );

    let canonical_cbor = build_phase1_canonical_cbor(&args)?;
    let computed_receipt_hash = hash(&canonical_cbor).to_bytes();
    require!(
        computed_receipt_hash == args.receipt_hash,
        ErrorCode::ReceiptHashMismatch
    );

    verify_preceding_ed25519_instruction(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &args.signer,
        &args.signature,
        &args.receipt_hash,
    )?;

    let receipt = &mut ctx.accounts.receipt;
    receipt.request_nonce = args.request_nonce.clone();
    receipt.proof_id = args.proof_id.clone();
    receipt.provider = args.provider.clone();
    receipt.model = args.model.clone();
    receipt.proof_mode = args.proof_mode;
    receipt.attester_type = args.attester_type;
    receipt.usage_basis = args.usage_basis;
    receipt.prompt_tokens = args.prompt_tokens;
    receipt.completion_tokens = args.completion_tokens;
    receipt.total_tokens = args.total_tokens;
    receipt.charge_atomic = args.charge_atomic;
    receipt.charge_mint = args.charge_mint;
    receipt.receipt_hash = args.receipt_hash;
    receipt.signer = args.signer;
    receipt.proof_url_hash = hash(args.proof_url.as_bytes()).to_bytes();
    receipt.submitted_at = now;
    receipt.challenge_deadline = now.saturating_add(config.challenge_window_seconds);
    receipt.finalized_at = 0;
    receipt.status = ReceiptStatus::Submitted as u8;
    receipt.bump = ctx.bumps.receipt;
    receipt.reserved = [0; 64];

    config.receipt_count = config.receipt_count.saturating_add(1);

    emit!(ReceiptSubmitted {
        request_nonce: args.request_nonce,
        proof_id: args.proof_id,
        provider: args.provider,
        signer: args.signer,
        receipt_hash: args.receipt_hash,
    });
    Ok(())
}

pub fn finalize_receipt(ctx: Context<FinalizeReceipt>, request_nonce: String) -> Result<()> {
    validate_request_nonce(&request_nonce)?;
    let now = Clock::get()?.unix_timestamp;
    let receipt = &mut ctx.accounts.receipt;

    require!(
        receipt.request_nonce == request_nonce,
        ErrorCode::ReceiptNonceMismatch
    );
    require!(
        receipt.status == ReceiptStatus::Submitted as u8,
        ErrorCode::ReceiptNotFinalizable
    );
    require!(
        now > receipt.challenge_deadline,
        ErrorCode::ChallengeWindowOpen
    );

    receipt.status = ReceiptStatus::Finalized as u8;
    receipt.finalized_at = now;

    emit!(ReceiptFinalized {
        request_nonce,
        proof_id: receipt.proof_id.clone(),
        provider: receipt.provider.clone(),
        signer: receipt.signer,
        receipt_hash: receipt.receipt_hash,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: SubmitReceiptArgs)]
pub struct SubmitReceipt<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [
            PROVIDER_SIGNER_SEED,
            &provider_signer_seed(args.provider.as_str()),
            args.signer.as_ref()
        ],
        bump = provider_signer.bump
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
    #[account(
        init,
        payer = payer,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, &request_nonce_seed(args.request_nonce.as_str())],
        bump
    )]
    pub receipt: Account<'info, Receipt>,
    /// CHECK: validated against the instructions sysvar id
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(request_nonce: String)]
pub struct FinalizeReceipt<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [RECEIPT_SEED, &request_nonce_seed(request_nonce.as_str())],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
}
