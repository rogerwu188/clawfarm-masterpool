use anchor_lang::prelude::*;

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

    receipt.status = ReceiptStatus::Challenged as u8;

    emit!(ChallengeOpened {
        challenge: challenge.key(),
        receipt: receipt.key(),
        challenger: ctx.accounts.challenger.key(),
        challenge_type: challenge_type as u8,
    });
    Ok(())
}

pub fn resolve_challenge(ctx: Context<ResolveChallenge>, resolution_code: u8) -> Result<()> {
    let resolution_code = ResolutionCode::try_from(resolution_code)?;
    require!(
        resolution_code != ResolutionCode::None,
        ErrorCode::ChallengeResolutionInvalid
    );

    let challenge = &mut ctx.accounts.challenge;
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
        challenge: challenge.key(),
        receipt: receipt.key(),
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
            || challenge.status == ChallengeStatus::Rejected as u8
            || challenge.status == ChallengeStatus::Expired as u8,
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
    #[account(mut)]
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
pub struct ResolveChallenge<'info> {
    pub challenge_resolver: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = challenge_resolver
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        has_one = receipt @ ErrorCode::ReceiptNonceMismatch
    )]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
pub struct CloseChallenge<'info> {
    #[account(mut)]
    pub recipient: Signer<'info>,
    #[account(
        mut,
        close = recipient
    )]
    pub challenge: Account<'info, Challenge>,
}
