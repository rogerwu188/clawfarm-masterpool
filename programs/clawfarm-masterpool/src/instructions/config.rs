use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::solana_program::program_option::COption;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{
    self, Mint, MintTo, SetAuthority, Token, TokenAccount,
};

use crate::{
    constants::{
        CHALLENGE_BOND_VAULT_SEED, CONFIG_SEED, GENESIS_TOTAL_SUPPLY, POOL_AUTHORITY_SEED,
        PROVIDER_PENDING_USDC_VAULT_SEED, PROVIDER_STAKE_USDC_VAULT_SEED, REWARD_VAULT_SEED,
        TREASURY_USDC_VAULT_SEED,
    },
    error::ErrorCode,
    state::{GlobalConfig, Phase1ConfigParams, GLOBAL_CONFIG_SPACE},
    utils::{require_phase1_mint_decimals, validate_phase1_params},
};

pub fn initialize_masterpool(
    ctx: Context<InitializeMasterpool>,
    params: Phase1ConfigParams,
) -> Result<()> {
    validate_phase1_params(&params)?;
    require!(
        ctx.accounts.attestation_program.executable,
        ErrorCode::InvalidAttestationProgram
    );
    require_phase1_mint_decimals(&ctx.accounts.claw_mint, &ctx.accounts.usdc_mint)?;

    let now = Clock::get()?.unix_timestamp;
    let config = &mut ctx.accounts.config;
    config.version = 1;
    config.admin_authority = ctx.accounts.admin.key();
    config.attestation_program = ctx.accounts.attestation_program.key();
    config.claw_mint = ctx.accounts.claw_mint.key();
    config.usdc_mint = ctx.accounts.usdc_mint.key();
    config.reward_vault = ctx.accounts.reward_vault.key();
    config.challenge_bond_vault = ctx.accounts.challenge_bond_vault.key();
    config.treasury_usdc_vault = ctx.accounts.treasury_usdc_vault.key();
    config.provider_stake_usdc_vault = ctx.accounts.provider_stake_usdc_vault.key();
    config.provider_pending_usdc_vault = ctx.accounts.provider_pending_usdc_vault.key();
    apply_phase1_params(config, &params);
    config.genesis_minted = false;
    config.pause_receipt_recording = false;
    config.pause_challenge_processing = false;
    config.pause_finalization = false;
    config.pause_claims = false;
    config.created_at = now;
    config.updated_at = now;

    Ok(())
}

pub fn mint_genesis_supply(ctx: Context<MintGenesisSupply>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.genesis_minted, ErrorCode::GenesisAlreadyMinted);
    require!(
        ctx.accounts.claw_mint.mint_authority
            == COption::Some(ctx.accounts.pool_authority.key()),
        ErrorCode::InvalidClawMintAuthority
    );
    require!(
        ctx.accounts.claw_mint.freeze_authority
            == COption::Some(ctx.accounts.pool_authority.key()),
        ErrorCode::InvalidClawFreezeAuthority
    );

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.claw_mint.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer,
        ),
        GENESIS_TOTAL_SUPPLY,
    )?;

    token::set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            SetAuthority {
                current_authority: ctx.accounts.pool_authority.to_account_info(),
                account_or_mint: ctx.accounts.claw_mint.to_account_info(),
            },
            signer,
        ),
        AuthorityType::MintTokens,
        None,
    )?;

    token::set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            SetAuthority {
                current_authority: ctx.accounts.pool_authority.to_account_info(),
                account_or_mint: ctx.accounts.claw_mint.to_account_info(),
            },
            signer,
        ),
        AuthorityType::FreezeAccount,
        None,
    )?;

    config.genesis_minted = true;
    config.updated_at = Clock::get()?.unix_timestamp;

    Ok(())
}

pub fn update_config(ctx: Context<UpdateConfig>, params: Phase1ConfigParams) -> Result<()> {
    validate_phase1_params(&params)?;
    let config = &mut ctx.accounts.config;
    apply_phase1_params(config, &params);
    config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn set_pause_flags(
    ctx: Context<SetPauseFlags>,
    pause_receipt_recording: bool,
    pause_challenge_processing: bool,
    pause_finalization: bool,
    pause_claims: bool,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.pause_receipt_recording = pause_receipt_recording;
    config.pause_challenge_processing = pause_challenge_processing;
    config.pause_finalization = pause_finalization;
    config.pause_claims = pause_claims;
    config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

fn apply_phase1_params(config: &mut GlobalConfig, params: &Phase1ConfigParams) {
    config.exchange_rate_claw_per_usdc_e6 = params.exchange_rate_claw_per_usdc_e6;
    config.provider_stake_usdc = params.provider_stake_usdc;
    config.provider_usdc_share_bps = params.provider_usdc_share_bps;
    config.treasury_usdc_share_bps = params.treasury_usdc_share_bps;
    config.user_claw_share_bps = params.user_claw_share_bps;
    config.provider_claw_share_bps = params.provider_claw_share_bps;
    config.lock_days = params.lock_days;
    config.provider_slash_claw_amount = params.provider_slash_claw_amount;
    config.challenger_reward_bps = params.challenger_reward_bps;
    config.burn_bps = params.burn_bps;
    config.challenge_bond_claw_amount = params.challenge_bond_claw_amount;
}

#[derive(Accounts)]
pub struct InitializeMasterpool<'info> {
    #[account(
        init,
        payer = initializer,
        space = GLOBAL_CONFIG_SPACE,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        init,
        payer = initializer,
        token::mint = claw_mint,
        token::authority = pool_authority,
        seeds = [REWARD_VAULT_SEED],
        bump,
    )]
    pub reward_vault: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = initializer,
        token::mint = claw_mint,
        token::authority = pool_authority,
        seeds = [CHALLENGE_BOND_VAULT_SEED],
        bump,
    )]
    pub challenge_bond_vault: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = initializer,
        token::mint = usdc_mint,
        token::authority = pool_authority,
        seeds = [TREASURY_USDC_VAULT_SEED],
        bump,
    )]
    pub treasury_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = initializer,
        token::mint = usdc_mint,
        token::authority = pool_authority,
        seeds = [PROVIDER_STAKE_USDC_VAULT_SEED],
        bump,
    )]
    pub provider_stake_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = initializer,
        token::mint = usdc_mint,
        token::authority = pool_authority,
        seeds = [PROVIDER_PENDING_USDC_VAULT_SEED],
        bump,
    )]
    pub provider_pending_usdc_vault: Account<'info, TokenAccount>,
    pub claw_mint: Account<'info, Mint>,
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: only the executable bit is required here
    pub attestation_program: UncheckedAccount<'info>,
    #[account(
        constraint = self_program.programdata_address()? == Some(self_program_data.key())
            @ ErrorCode::InvalidProgramData,
    )]
    pub self_program: Program<'info, crate::program::ClawfarmMasterpool>,
    #[account(
        constraint = self_program_data.upgrade_authority_address == Some(initializer.key())
            @ ErrorCode::UnauthorizedInitializer,
    )]
    pub self_program_data: Account<'info, ProgramData>,
    /// CHECK: PDA signer for program-owned vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub initializer: Signer<'info>,
    /// CHECK: operational authority recorded in config
    pub admin: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintGenesisSupply<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
        has_one = claw_mint @ ErrorCode::InvalidClawMint,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    pub admin_authority: Signer<'info>,
    #[account(
        mut,
        address = config.reward_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub reward_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    /// CHECK: PDA signer for the reward vault
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    pub admin_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetPauseFlags<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    pub admin_authority: Signer<'info>,
}
