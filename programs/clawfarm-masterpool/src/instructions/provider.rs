use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, Token, TokenAccount, TransferChecked,
};

use crate::{
    constants::{
        CONFIG_SEED, POOL_AUTHORITY_SEED, PROVIDER_REWARD_SEED, PROVIDER_SEED,
    },
    error::ErrorCode,
    state::{
        GlobalConfig, ProviderAccount, RewardAccount, RewardAccountKind, ProviderStatus,
        PROVIDER_ACCOUNT_SPACE, REWARD_ACCOUNT_SPACE,
    },
    utils::{
        checked_sub_u64, initialize_reward_account, require_token_mint, require_token_owner,
    },
};

pub fn register_provider(ctx: Context<RegisterProvider>) -> Result<()> {
    let config = &ctx.accounts.config;
    let provider_wallet = ctx.accounts.provider_wallet.key();
    require_token_owner(&ctx.accounts.provider_usdc_token, &provider_wallet)?;
    require_token_mint(&ctx.accounts.provider_usdc_token, &config.usdc_mint)?;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.provider_usdc_token.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.provider_stake_usdc_vault.to_account_info(),
                authority: ctx.accounts.provider_wallet.to_account_info(),
            },
        ),
        config.provider_stake_usdc,
        ctx.accounts.usdc_mint.decimals,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let provider = &mut ctx.accounts.provider_account;
    provider.provider_wallet = provider_wallet;
    provider.staked_usdc_amount = config.provider_stake_usdc;
    provider.pending_provider_usdc = 0;
    provider.claw_net_position = 0;
    provider.unsettled_receipt_count = 0;
    provider.unresolved_challenge_count = 0;
    provider.status = ProviderStatus::Active.into();
    provider.created_at = now;
    provider.updated_at = now;

    let reward_account = &mut ctx.accounts.provider_reward_account;
    initialize_reward_account(
        reward_account,
        provider_wallet,
        RewardAccountKind::Provider,
        now,
    )?;

    Ok(())
}

pub fn exit_provider(ctx: Context<ExitProvider>) -> Result<()> {
    let provider = &mut ctx.accounts.provider_account;
    require!(
        provider.status == u8::from(ProviderStatus::Active),
        ErrorCode::ProviderNotActive
    );
    require!(
        provider.pending_provider_usdc == 0
            && provider.unsettled_receipt_count == 0
            && provider.unresolved_challenge_count == 0
            && provider.claw_net_position >= 0,
        ErrorCode::ProviderExitBlocked
    );

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.provider_stake_usdc_vault.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.provider_destination_usdc.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer,
        ),
        provider.staked_usdc_amount,
        ctx.accounts.usdc_mint.decimals,
    )?;

    provider.staked_usdc_amount = checked_sub_u64(provider.staked_usdc_amount, provider.staked_usdc_amount)?;
    provider.status = ProviderStatus::Exited.into();
    provider.updated_at = Clock::get()?.unix_timestamp;

    Ok(())
}

#[derive(Accounts)]
pub struct RegisterProvider<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        init,
        payer = provider_wallet,
        space = PROVIDER_ACCOUNT_SPACE,
        seeds = [PROVIDER_SEED, provider_wallet.key().as_ref()],
        bump,
    )]
    pub provider_account: Account<'info, ProviderAccount>,
    #[account(
        init,
        payer = provider_wallet,
        space = REWARD_ACCOUNT_SPACE,
        seeds = [PROVIDER_REWARD_SEED, provider_wallet.key().as_ref()],
        bump,
    )]
    pub provider_reward_account: Account<'info, RewardAccount>,
    #[account(mut)]
    pub provider_wallet: Signer<'info>,
    #[account(
        mut,
        address = config.provider_stake_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub provider_stake_usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub provider_usdc_token: Account<'info, TokenAccount>,
    #[account(
        constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExitProvider<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [PROVIDER_SEED, provider_wallet.key().as_ref()],
        bump,
        constraint = provider_account.provider_wallet == provider_wallet.key() @ ErrorCode::InvalidProviderAccount,
    )]
    pub provider_account: Account<'info, ProviderAccount>,
    #[account(mut)]
    pub provider_wallet: Signer<'info>,
    #[account(
        mut,
        address = config.provider_stake_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub provider_stake_usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub provider_destination_usdc: Account<'info, TokenAccount>,
    #[account(
        constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: PDA signer for the stake vault
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}
