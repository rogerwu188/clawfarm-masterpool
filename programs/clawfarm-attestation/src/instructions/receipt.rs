use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use clawfarm_masterpool::{
    self,
    cpi::accounts::{RecordMiningFromReceipt as MasterpoolRecordMiningFromReceipt, SettleFinalizedReceipt as MasterpoolSettleFinalizedReceipt},
    program::ClawfarmMasterpool,
    RecordMiningFromReceiptArgs,
};
use solana_sdk_ids::sysvar::instructions::ID as INSTRUCTIONS_SYSVAR_ID;
use solana_sha256_hasher::hash;

use crate::{
    constants::{CONFIG_SEED, PROVIDER_SIGNER_SEED, RECEIPT_SEED},
    error::ErrorCode,
    events::{ReceiptClosed, ReceiptFinalized, ReceiptSubmitted},
    state::{Config, ProviderSigner, Receipt, ReceiptStatus, SignerStatus, SubmitReceiptArgs},
    utils::{
        attester_type_mask, build_phase1_canonical_cbor, provider_signer_seed, request_nonce_seed,
        validate_submit_receipt_args, verify_preceding_ed25519_instruction,
    },
};

pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()> {
    validate_submit_receipt_args(&args)?;

    let now = Clock::get()?.unix_timestamp;
    let config = &ctx.accounts.config;
    require!(!config.is_paused, ErrorCode::ProgramPaused);

    let provider_signer = &ctx.accounts.provider_signer;
    require!(
        provider_signer.status == SignerStatus::Active as u8,
        ErrorCode::SignerInactive
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
        &args.receipt_hash,
    )?;

    let receipt_info = ctx.accounts.receipt.to_account_info();
    let receipt_key = ctx.accounts.receipt.key();
    let receipt = &mut ctx.accounts.receipt;
    receipt.receipt_hash = args.receipt_hash;
    receipt.signer = args.signer;
    receipt.submitted_at = now;
    receipt.challenge_deadline = now.saturating_add(config.challenge_window_seconds);
    receipt.finalized_at = 0;
    receipt.status = ReceiptStatus::Submitted as u8;
    receipt.economics_settled = false;

    let signer_seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];
    clawfarm_masterpool::cpi::record_mining_from_receipt(
        CpiContext::new_with_signer(
            ctx.accounts.masterpool_program.to_account_info(),
            MasterpoolRecordMiningFromReceipt {
                config: ctx.accounts.masterpool_config.to_account_info(),
                attestation_config: ctx.accounts.config.to_account_info(),
                payer_user: ctx.accounts.payer_user.to_account_info(),
                payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
                provider_wallet: ctx.accounts.provider_wallet.to_account_info(),
                provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
                provider_reward_account: ctx.accounts.masterpool_provider_reward_account.to_account_info(),
                user_reward_account: ctx.accounts.masterpool_user_reward_account.to_account_info(),
                receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
                attestation_receipt: receipt_info,
                treasury_usdc_vault: ctx.accounts.masterpool_treasury_usdc_vault.to_account_info(),
                provider_pending_usdc_vault: ctx.accounts.masterpool_provider_pending_usdc_vault.to_account_info(),
                usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            &[signer_seeds],
        ),
        RecordMiningFromReceiptArgs {
            total_usdc_paid: args.charge_atomic,
            charge_mint: args.charge_mint,
        },
    )?;

    emit!(ReceiptSubmitted {
        receipt: receipt_key,
        request_nonce: args.request_nonce,
        proof_id: args.proof_id,
        provider: args.provider,
        signer: args.signer,
        receipt_hash: args.receipt_hash,
        challenge_deadline: receipt.challenge_deadline,
    });
    Ok(())
}

pub fn finalize_receipt(ctx: Context<FinalizeReceipt>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let receipt_info = ctx.accounts.receipt.to_account_info();
    let receipt = &mut ctx.accounts.receipt;

    match receipt.status {
        x if x == ReceiptStatus::Submitted as u8 => {
            require!(
                now > receipt.challenge_deadline,
                ErrorCode::ChallengeWindowOpen
            );
            receipt.status = ReceiptStatus::Finalized as u8;
            receipt.finalized_at = now;
        }
        x if x == ReceiptStatus::Finalized as u8 => {
            require!(
                !receipt.economics_settled,
                ErrorCode::ReceiptAlreadySettled
            );
        }
        _ => return err!(ErrorCode::ReceiptNotFinalizable),
    }

    let signer_seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];
    clawfarm_masterpool::cpi::settle_finalized_receipt(
        CpiContext::new_with_signer(
            ctx.accounts.masterpool_program.to_account_info(),
            MasterpoolSettleFinalizedReceipt {
                config: ctx.accounts.masterpool_config.to_account_info(),
                attestation_config: ctx.accounts.config.to_account_info(),
                attestation_receipt: receipt_info,
                receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
                provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
                provider_pending_usdc_vault: ctx.accounts.masterpool_provider_pending_usdc_vault.to_account_info(),
                provider_destination_usdc: ctx.accounts.masterpool_provider_destination_usdc.to_account_info(),
                usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                pool_authority: ctx.accounts.masterpool_pool_authority.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
            &[signer_seeds],
        ),
        ReceiptStatus::Finalized as u8,
    )?;

    receipt.economics_settled = true;

    emit!(ReceiptFinalized {
        receipt: receipt.key(),
        signer: receipt.signer,
        receipt_hash: receipt.receipt_hash,
    });
    Ok(())
}

pub fn close_receipt(ctx: Context<CloseReceipt>) -> Result<()> {
    let receipt = &ctx.accounts.receipt;
    require!(
        is_terminal_receipt_status(receipt.status),
        ErrorCode::ReceiptNotClosable
    );
    require!(
        receipt.economics_settled,
        ErrorCode::ReceiptEconomicsPending
    );

    emit!(ReceiptClosed {
        receipt: receipt.key(),
        signer: receipt.signer,
        receipt_hash: receipt.receipt_hash,
        status: receipt.status,
    });
    Ok(())
}

fn is_terminal_receipt_status(status: u8) -> bool {
    matches!(
        status,
        x if x == ReceiptStatus::Finalized as u8
            || x == ReceiptStatus::Rejected as u8
            || x == ReceiptStatus::Slashed as u8
    )
}

#[derive(Accounts)]
#[instruction(args: SubmitReceiptArgs)]
pub struct SubmitReceipt<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        has_one = authority,
        has_one = masterpool_program,
        bump
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(
        seeds = [
            PROVIDER_SIGNER_SEED,
            &provider_signer_seed(args.provider.as_str()),
            args.signer.as_ref()
        ],
        bump
    )]
    pub provider_signer: Box<Account<'info, ProviderSigner>>,
    #[account(
        init,
        payer = authority,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, &request_nonce_seed(args.request_nonce.as_str())],
        bump
    )]
    pub receipt: Box<Account<'info, Receipt>>,
    #[account(mut)]
    pub payer_user: Signer<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub payer_usdc_token: UncheckedAccount<'info>,
    /// CHECK: provider wallet identity is validated downstream by masterpool
    pub provider_wallet: UncheckedAccount<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        seeds::program = masterpool_program,
        bump
    )]
    /// CHECK: validated by masterpool
    pub masterpool_config: UncheckedAccount<'info>,
    pub masterpool_program: Program<'info, ClawfarmMasterpool>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_provider_account: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_provider_reward_account: UncheckedAccount<'info>,
    /// CHECK: masterpool initializes or validates this account
    #[account(mut)]
    pub masterpool_user_reward_account: UncheckedAccount<'info>,
    /// CHECK: masterpool initializes this account
    #[account(mut)]
    pub masterpool_receipt_settlement: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_treasury_usdc_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_provider_pending_usdc_vault: UncheckedAccount<'info>,
    /// CHECK: validated by masterpool
    pub usdc_mint: UncheckedAccount<'info>,
    /// CHECK: validated against the instructions sysvar id
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeReceipt<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        has_one = authority,
        has_one = masterpool_program,
        bump
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(mut)]
    pub receipt: Box<Account<'info, Receipt>>,
    #[account(
        seeds = [CONFIG_SEED],
        seeds::program = masterpool_program,
        bump
    )]
    /// CHECK: validated by masterpool
    pub masterpool_config: UncheckedAccount<'info>,
    pub masterpool_program: Program<'info, ClawfarmMasterpool>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_receipt_settlement: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_provider_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_provider_pending_usdc_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_provider_destination_usdc: UncheckedAccount<'info>,
    /// CHECK: validated by masterpool
    pub usdc_mint: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this PDA
    pub masterpool_pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseReceipt<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        has_one = authority,
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        close = authority
    )]
    pub receipt: Account<'info, Receipt>,
}
