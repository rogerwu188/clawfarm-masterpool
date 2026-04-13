use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, Token, TokenAccount, TransferChecked,
};

use crate::{
    constants::{CONFIG_SEED, POOL_AUTHORITY_SEED},
    error::ErrorCode,
    state::{GlobalConfig, RewardAccount},
    utils::{checked_add_u64, checked_sub_u64, require_token_mint, require_token_owner},
};

pub fn materialize_reward_release(
    ctx: Context<MaterializeRewardRelease>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidPositiveAmount);
    let reward_account = &mut ctx.accounts.reward_account;
    require!(
        reward_account.locked_claw_total >= amount,
        ErrorCode::RewardReleaseExceedsLocked
    );

    reward_account.locked_claw_total = checked_sub_u64(reward_account.locked_claw_total, amount)?;
    reward_account.released_claw_total = checked_add_u64(reward_account.released_claw_total, amount)?;
    reward_account.updated_at = Clock::get()?.unix_timestamp;

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
