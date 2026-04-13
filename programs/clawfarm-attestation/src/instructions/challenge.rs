use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use clawfarm_masterpool::{
    self,
    cpi::accounts::{RecordChallengeBond as MasterpoolRecordChallengeBond, ResolveChallengeEconomics as MasterpoolResolveChallengeEconomics},
    program::ClawfarmMasterpool,
    GlobalConfig as MasterpoolConfig,
};

use crate::{
    constants::{CHALLENGE_SEED, CONFIG_SEED},
    error::ErrorCode,
    events::{ChallengeClosed, ChallengeOpened, ChallengeResolved},
    state::{
        Challenge, ChallengeStatus, ChallengeType, Config, Receipt, ReceiptStatus, ResolutionCode,
    },
};

pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()> {
    let challenge_type = ChallengeType::try_from(challenge_type)?;
    let now = Clock::get()?.unix_timestamp;
    let receipt_info = ctx.accounts.receipt.to_account_info();
    let challenge_info = ctx.accounts.challenge.to_account_info();
    let receipt_key = ctx.accounts.receipt.key();
    let challenge_key = ctx.accounts.challenge.key();

    let receipt = &mut ctx.accounts.receipt;
    require!(
        receipt.status == ReceiptStatus::Submitted as u8,
        ErrorCode::ReceiptNotChallengeable
    );
    require!(
        now <= receipt.challenge_deadline,
        ErrorCode::ChallengeWindowClosed
    );

    let signer_seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];
    clawfarm_masterpool::cpi::record_challenge_bond(CpiContext::new_with_signer(
        ctx.accounts.masterpool_program.to_account_info(),
        MasterpoolRecordChallengeBond {
            config: ctx.accounts.masterpool_config.to_account_info(),
            attestation_config: ctx.accounts.config.to_account_info(),
            challenger: ctx.accounts.challenger.to_account_info(),
            challenger_claw_token: ctx.accounts.challenger_claw_token.to_account_info(),
            attestation_receipt: receipt_info,
            attestation_challenge: challenge_info,
            receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
            provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
            challenge_bond_record: ctx.accounts.masterpool_challenge_bond_record.to_account_info(),
            challenge_bond_vault: ctx.accounts.masterpool_challenge_bond_vault.to_account_info(),
            claw_mint: ctx.accounts.claw_mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        &[signer_seeds],
    ))?;

    let challenge = &mut ctx.accounts.challenge;
    challenge.receipt = receipt_key;
    challenge.challenger = ctx.accounts.challenger.key();
    challenge.challenge_type = challenge_type as u8;
    challenge.evidence_hash = evidence_hash;
    challenge.bond_amount = ctx.accounts.masterpool_config.challenge_bond_claw_amount;
    challenge.opened_at = now;
    challenge.resolved_at = 0;
    challenge.status = ChallengeStatus::Open as u8;
    challenge.resolution_code = ResolutionCode::None as u8;

    receipt.status = ReceiptStatus::Challenged as u8;

    emit!(ChallengeOpened {
        challenge: challenge_key,
        receipt: receipt_key,
        challenger: ctx.accounts.challenger.key(),
        challenge_type: challenge_type as u8,
        bond_amount: challenge.bond_amount,
    });
    Ok(())
}

pub fn resolve_challenge(ctx: Context<ResolveChallenge>, resolution_code: u8) -> Result<()> {
    let resolution_code = ResolutionCode::try_from(resolution_code)?;
    require!(
        resolution_code != ResolutionCode::None,
        ErrorCode::ChallengeResolutionInvalid
    );

    let receipt_info = ctx.accounts.receipt.to_account_info();
    let challenge_info = ctx.accounts.challenge.to_account_info();
    let receipt_key = ctx.accounts.receipt.key();
    let challenge_key = ctx.accounts.challenge.key();
    let challenge = &mut ctx.accounts.challenge;
    require!(
        challenge.status == ChallengeStatus::Open as u8,
        ErrorCode::ChallengeNotResolvable
    );

    challenge.resolution_code = resolution_code as u8;
    let now = Clock::get()?.unix_timestamp;
    challenge.resolved_at = now;

    let receipt = &mut ctx.accounts.receipt;
    let signer_seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];

    match resolution_code {
        ResolutionCode::Accepted | ResolutionCode::ReceiptInvalidated => {
            challenge.status = ChallengeStatus::Accepted as u8;
            receipt.status = ReceiptStatus::Rejected as u8;
            receipt.finalized_at = now;

            clawfarm_masterpool::cpi::resolve_challenge_economics(
                CpiContext::new_with_signer(
                    ctx.accounts.masterpool_program.to_account_info(),
                    MasterpoolResolveChallengeEconomics {
                        config: ctx.accounts.masterpool_config.to_account_info(),
                        attestation_config: ctx.accounts.config.to_account_info(),
                        attestation_receipt: receipt_info.clone(),
                        attestation_challenge: challenge_info.clone(),
                        receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
                        challenge_bond_record: ctx.accounts.masterpool_challenge_bond_record.to_account_info(),
                        provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
                        challenge_bond_vault: ctx.accounts.masterpool_challenge_bond_vault.to_account_info(),
                        reward_vault: ctx.accounts.masterpool_reward_vault.to_account_info(),
                        provider_pending_usdc_vault: ctx.accounts.masterpool_provider_pending_usdc_vault.to_account_info(),
                        challenger_claw_token: ctx.accounts.challenger_claw_token.to_account_info(),
                        payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
                        claw_mint: ctx.accounts.claw_mint.to_account_info(),
                        usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                        pool_authority: ctx.accounts.masterpool_pool_authority.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    },
                    &[signer_seeds],
                ),
                resolution_code as u8,
            )?;
            receipt.economics_settled = true;
        }
        ResolutionCode::SignerRevoked => {
            challenge.status = ChallengeStatus::Accepted as u8;
            receipt.status = ReceiptStatus::Slashed as u8;
            receipt.finalized_at = now;

            clawfarm_masterpool::cpi::resolve_challenge_economics(
                CpiContext::new_with_signer(
                    ctx.accounts.masterpool_program.to_account_info(),
                    MasterpoolResolveChallengeEconomics {
                        config: ctx.accounts.masterpool_config.to_account_info(),
                        attestation_config: ctx.accounts.config.to_account_info(),
                        attestation_receipt: receipt_info.clone(),
                        attestation_challenge: challenge_info.clone(),
                        receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
                        challenge_bond_record: ctx.accounts.masterpool_challenge_bond_record.to_account_info(),
                        provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
                        challenge_bond_vault: ctx.accounts.masterpool_challenge_bond_vault.to_account_info(),
                        reward_vault: ctx.accounts.masterpool_reward_vault.to_account_info(),
                        provider_pending_usdc_vault: ctx.accounts.masterpool_provider_pending_usdc_vault.to_account_info(),
                        challenger_claw_token: ctx.accounts.challenger_claw_token.to_account_info(),
                        payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
                        claw_mint: ctx.accounts.claw_mint.to_account_info(),
                        usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                        pool_authority: ctx.accounts.masterpool_pool_authority.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    },
                    &[signer_seeds],
                ),
                resolution_code as u8,
            )?;
            receipt.economics_settled = true;
        }
        ResolutionCode::Rejected => {
            challenge.status = ChallengeStatus::Rejected as u8;
            receipt.status = ReceiptStatus::Finalized as u8;
            receipt.finalized_at = now;

            clawfarm_masterpool::cpi::resolve_challenge_economics(
                CpiContext::new_with_signer(
                    ctx.accounts.masterpool_program.to_account_info(),
                    MasterpoolResolveChallengeEconomics {
                        config: ctx.accounts.masterpool_config.to_account_info(),
                        attestation_config: ctx.accounts.config.to_account_info(),
                        attestation_receipt: receipt_info,
                        attestation_challenge: challenge_info,
                        receipt_settlement: ctx.accounts.masterpool_receipt_settlement.to_account_info(),
                        challenge_bond_record: ctx.accounts.masterpool_challenge_bond_record.to_account_info(),
                        provider_account: ctx.accounts.masterpool_provider_account.to_account_info(),
                        challenge_bond_vault: ctx.accounts.masterpool_challenge_bond_vault.to_account_info(),
                        reward_vault: ctx.accounts.masterpool_reward_vault.to_account_info(),
                        provider_pending_usdc_vault: ctx.accounts.masterpool_provider_pending_usdc_vault.to_account_info(),
                        challenger_claw_token: ctx.accounts.challenger_claw_token.to_account_info(),
                        payer_usdc_token: ctx.accounts.payer_usdc_token.to_account_info(),
                        claw_mint: ctx.accounts.claw_mint.to_account_info(),
                        usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                        pool_authority: ctx.accounts.masterpool_pool_authority.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    },
                    &[signer_seeds],
                ),
                resolution_code as u8,
            )?;
            receipt.economics_settled = false;
        }
        ResolutionCode::None => unreachable!(),
    }

    emit!(ChallengeResolved {
        challenge: challenge_key,
        receipt: receipt_key,
        challenger: challenge.challenger,
        challenge_type: challenge.challenge_type,
        resolution_code: resolution_code as u8,
    });
    Ok(())
}

pub fn close_challenge(ctx: Context<CloseChallenge>) -> Result<()> {
    let challenge = &ctx.accounts.challenge;
    require!(
        challenge.status == ChallengeStatus::Accepted as u8
            || challenge.status == ChallengeStatus::Rejected as u8,
        ErrorCode::ChallengeNotClosable
    );

    emit!(ChallengeClosed {
        challenge: challenge.key(),
        receipt: challenge.receipt,
        challenger: challenge.challenger,
        challenge_type: challenge.challenge_type,
        resolution_code: challenge.resolution_code,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(challenge_type: u8)]
pub struct OpenChallenge<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        has_one = masterpool_program,
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub receipt: Account<'info, Receipt>,
    #[account(
        init,
        payer = challenger,
        space = 8 + Challenge::INIT_SPACE,
        seeds = [CHALLENGE_SEED, receipt.key().as_ref()],
        bump
    )]
    pub challenge: Account<'info, Challenge>,
    #[account(mut)]
    pub challenger_claw_token: Account<'info, TokenAccount>,
    #[account(
        seeds = [CONFIG_SEED],
        seeds::program = masterpool_program,
        bump
    )]
    pub masterpool_config: Account<'info, MasterpoolConfig>,
    pub masterpool_program: Program<'info, ClawfarmMasterpool>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_receipt_settlement: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_provider_account: UncheckedAccount<'info>,
    /// CHECK: masterpool initializes this account
    #[account(mut)]
    pub masterpool_challenge_bond_record: UncheckedAccount<'info>,
    #[account(mut)]
    pub masterpool_challenge_bond_vault: Account<'info, TokenAccount>,
    pub claw_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveChallenge<'info> {
    pub challenge_resolver: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = challenge_resolver,
        has_one = masterpool_program
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(mut)]
    pub receipt: Box<Account<'info, Receipt>>,
    #[account(
        mut,
        has_one = receipt @ ErrorCode::ReceiptNonceMismatch
    )]
    pub challenge: Box<Account<'info, Challenge>>,
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
    pub masterpool_challenge_bond_record: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this account
    #[account(mut)]
    pub masterpool_provider_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_challenge_bond_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_reward_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub masterpool_provider_pending_usdc_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub challenger_claw_token: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub payer_usdc_token: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated by masterpool
    pub claw_mint: UncheckedAccount<'info>,
    /// CHECK: validated by masterpool
    pub usdc_mint: UncheckedAccount<'info>,
    /// CHECK: masterpool validates this PDA
    pub masterpool_pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseChallenge<'info> {
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
    pub challenge: Account<'info, Challenge>,
}
