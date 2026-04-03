use anchor_lang::prelude::*;

use crate::{
    constants::{CHALLENGE_SEED, CONFIG_SEED, RECEIPT_SEED},
    error::ErrorCode,
    events::{ChallengeClosed, ChallengeOpened, ChallengeResolved},
    state::{
        Challenge, ChallengeStatus, ChallengeType, Config, Receipt, ReceiptStatus, ResolutionCode,
    },
    utils::{request_nonce_seed, validate_request_nonce},
};

pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    request_nonce: String,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()> {
    validate_request_nonce(&request_nonce)?;
    let challenge_type = ChallengeType::try_from(challenge_type)?;
    let now = Clock::get()?.unix_timestamp;

    let receipt = &mut ctx.accounts.receipt;
    require!(
        receipt.status == ReceiptStatus::Submitted as u8,
        ErrorCode::ReceiptNotChallengeable
    );
    require!(
        now <= receipt.challenge_deadline,
        ErrorCode::ChallengeWindowClosed
    );

    let challenge = &mut ctx.accounts.challenge;
    challenge.receipt = receipt.key();
    challenge.challenger = ctx.accounts.challenger.key();
    challenge.challenge_type = challenge_type as u8;
    challenge.evidence_hash = evidence_hash;
    challenge.opened_at = now;
    challenge.resolved_at = 0;
    challenge.status = ChallengeStatus::Open as u8;
    challenge.resolution_code = ResolutionCode::None as u8;
    challenge.bump = ctx.bumps.challenge;

    receipt.status = ReceiptStatus::Challenged as u8;
    ctx.accounts.config.challenge_count = ctx.accounts.config.challenge_count.saturating_add(1);

    emit!(ChallengeOpened {
        request_nonce,
        challenger: ctx.accounts.challenger.key(),
        challenge_type: challenge_type as u8,
    });
    Ok(())
}

pub fn resolve_challenge(
    ctx: Context<ResolveChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    resolution_code: u8,
) -> Result<()> {
    validate_request_nonce(&request_nonce)?;
    let challenge_type = ChallengeType::try_from(challenge_type)?;
    let resolution_code = ResolutionCode::try_from(resolution_code)?;
    require!(
        resolution_code != ResolutionCode::None,
        ErrorCode::ChallengeResolutionInvalid
    );

    let challenge = &mut ctx.accounts.challenge;
    require!(
        challenge.challenger == challenger,
        ErrorCode::ChallengeChallengerMismatch
    );
    require!(
        challenge.challenge_type == challenge_type as u8,
        ErrorCode::ChallengeTypeMismatch
    );
    require!(
        challenge.status == ChallengeStatus::Open as u8,
        ErrorCode::ChallengeNotResolvable
    );

    challenge.resolution_code = resolution_code as u8;
    let now = Clock::get()?.unix_timestamp;
    challenge.resolved_at = now;

    let receipt = &mut ctx.accounts.receipt;
    match resolution_code {
        ResolutionCode::Accepted | ResolutionCode::ReceiptInvalidated => {
            challenge.status = ChallengeStatus::Accepted as u8;
            receipt.status = ReceiptStatus::Rejected as u8;
            receipt.finalized_at = now;
        }
        ResolutionCode::SignerRevoked => {
            challenge.status = ChallengeStatus::Accepted as u8;
            receipt.status = ReceiptStatus::Slashed as u8;
            receipt.finalized_at = now;
        }
        ResolutionCode::Rejected => {
            challenge.status = ChallengeStatus::Rejected as u8;
            receipt.status = ReceiptStatus::Finalized as u8;
            receipt.finalized_at = now;
        }
        ResolutionCode::None => unreachable!(),
    }

    emit!(ChallengeResolved {
        request_nonce,
        challenger,
        challenge_type: challenge_type as u8,
        resolution_code: resolution_code as u8,
    });
    Ok(())
}

pub fn close_challenge(
    ctx: Context<CloseChallenge>,
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
) -> Result<()> {
    validate_request_nonce(&request_nonce)?;
    let challenge_type = ChallengeType::try_from(challenge_type)?;
    let challenge = &ctx.accounts.challenge;
    require!(
        challenge.status == ChallengeStatus::Accepted as u8
            || challenge.status == ChallengeStatus::Rejected as u8
            || challenge.status == ChallengeStatus::Expired as u8,
        ErrorCode::ChallengeNotClosable
    );

    emit!(ChallengeClosed {
        challenge: challenge.key(),
        receipt: challenge.receipt,
        challenger,
        challenge_type: challenge_type as u8,
        resolution_code: challenge.resolution_code,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(request_nonce: String, challenge_type: u8, evidence_hash: [u8; 32])]
pub struct OpenChallenge<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,
    #[account(
        mut,
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
    #[account(
        init,
        payer = challenger,
        space = 8 + Challenge::INIT_SPACE,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.key().as_ref()
        ],
        bump
    )]
    pub challenge: Account<'info, Challenge>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    resolution_code: u8
)]
pub struct ResolveChallenge<'info> {
    pub challenge_resolver: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = challenge_resolver
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [RECEIPT_SEED, &request_nonce_seed(request_nonce.as_str())],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.as_ref()
        ],
        bump = challenge.bump
    )]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
#[instruction(request_nonce: String, challenge_type: u8, challenger: Pubkey)]
pub struct CloseChallenge<'info> {
    #[account(mut)]
    pub recipient: Signer<'info>,
    #[account(
        mut,
        seeds = [RECEIPT_SEED, &request_nonce_seed(request_nonce.as_str())],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.as_ref()
        ],
        bump = challenge.bump,
        close = recipient
    )]
    pub challenge: Account<'info, Challenge>,
}
