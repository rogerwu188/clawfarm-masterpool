use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, Token, TokenAccount, TransferChecked,
};

use crate::{
    constants::{CONFIG_SEED, POOL_AUTHORITY_SEED},
    error::ErrorCode,
    state::{
        GlobalConfig, ReceiptSettlement, ReceiptSettlementStatus, ReleaseTarget, RewardAccount,
        RewardAccountKind,
    },
    utils::{
        checked_add_u64, checked_sub_u64, compute_linear_releasable_amount, require_token_mint,
        require_token_owner,
    },
};

pub fn materialize_reward_release(
    ctx: Context<MaterializeRewardRelease>,
    target: u8,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidPositiveAmount);
    let target = ReleaseTarget::try_from(target)?;
    let settlement = &mut ctx.accounts.receipt_settlement;
    require!(
        settlement.status == u8::from(ReceiptSettlementStatus::FinalizedSettled),
        ErrorCode::InvalidReceiptSettlementState
    );
    require!(
        settlement.reward_lock_started_at > 0,
        ErrorCode::RewardLockNotStarted
    );

    let (expected_owner, expected_kind, total_locked, released_so_far) = match target {
        ReleaseTarget::User => (
            settlement.payer_user,
            RewardAccountKind::User,
            settlement.claw_to_user,
            settlement.user_claw_released,
        ),
        ReleaseTarget::Provider => (
            settlement.provider_wallet,
            RewardAccountKind::Provider,
            settlement.claw_to_provider_locked,
            settlement.provider_claw_released,
        ),
    };

    let now = Clock::get()?.unix_timestamp;
    let releasable = compute_linear_releasable_amount(
        total_locked,
        released_so_far,
        settlement.reward_lock_started_at,
        now,
        settlement.lock_days_snapshot,
    )?;
    require!(
        releasable >= amount,
        ErrorCode::RewardReleaseExceedsVested
    );

    let reward_account = &mut ctx.accounts.reward_account;
    require!(
        reward_account.owner == expected_owner,
        ErrorCode::InvalidRewardAccountOwner
    );
    require!(
        reward_account.reward_kind == u8::from(expected_kind),
        ErrorCode::InvalidRewardAccountKind
    );
    require!(
        reward_account.locked_claw_total >= amount,
        ErrorCode::RewardReleaseExceedsLocked
    );

    reward_account.locked_claw_total =
        checked_sub_u64(reward_account.locked_claw_total, amount)?;
    reward_account.released_claw_total =
        checked_add_u64(reward_account.released_claw_total, amount)?;
    reward_account.updated_at = now;

    match target {
        ReleaseTarget::User => {
            settlement.user_claw_released =
                checked_add_u64(settlement.user_claw_released, amount)?;
        }
        ReleaseTarget::Provider => {
            settlement.provider_claw_released =
                checked_add_u64(settlement.provider_claw_released, amount)?;
        }
    }
    settlement.updated_at = now;

    Ok(())
}

pub fn claim_released_claw(ctx: Context<ClaimReleasedClaw>) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.pause_claims, ErrorCode::ClaimsPaused);

    let reward_account = &mut ctx.accounts.reward_account;
    require!(
        reward_account.owner == ctx.accounts.claimant.key(),
        ErrorCode::InvalidRewardAccountOwner
    );

    let claimable = checked_sub_u64(
        reward_account.released_claw_total,
        reward_account.claimed_claw_total,
    )?;
    require!(claimable > 0, ErrorCode::NoClaimableRewards);

    require_token_owner(&ctx.accounts.claimant_claw_token, &ctx.accounts.claimant.key())?;
    require_token_mint(&ctx.accounts.claimant_claw_token, &config.claw_mint)?;

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.reward_vault.to_account_info(),
                mint: ctx.accounts.claw_mint.to_account_info(),
                to: ctx.accounts.claimant_claw_token.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer,
        ),
        claimable,
        ctx.accounts.claw_mint.decimals,
    )?;

    reward_account.claimed_claw_total = checked_add_u64(reward_account.claimed_claw_total, claimable)?;
    reward_account.updated_at = Clock::get()?.unix_timestamp;

    Ok(())
}

#[derive(Accounts)]
pub struct MaterializeRewardRelease<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub config: Account<'info, GlobalConfig>,
    pub admin_authority: Signer<'info>,
    #[account(mut)]
    pub reward_account: Account<'info, RewardAccount>,
    #[account(mut)]
    pub receipt_settlement: Account<'info, ReceiptSettlement>,
}

#[derive(Accounts)]
pub struct ClaimReleasedClaw<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(mut)]
    pub reward_account: Account<'info, RewardAccount>,
    #[account(mut)]
    pub claimant: Signer<'info>,
    #[account(
        mut,
        address = config.reward_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub reward_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub claimant_claw_token: Account<'info, TokenAccount>,
    #[account(
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    /// CHECK: PDA signer for the reward vault
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}
