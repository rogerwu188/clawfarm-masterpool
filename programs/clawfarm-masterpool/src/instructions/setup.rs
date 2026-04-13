use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Mint, MintTo, SetAuthority, Token, TokenAccount};

use crate::{
    constants::{
        COMPUTE_POOL_BPS, CONFIG_SEED, GENESIS_TOTAL_SUPPLY, MASTER_POOL_VAULT_SEED,
        OUTCOME_POOL_BPS, POOL_AUTHORITY_SEED, TREASURY_TAX_BPS, TREASURY_VAULT_SEED,
    },
    error::ErrorCode,
    state::ClawFarmConfig,
};

pub fn initialize_master_pool(
    ctx: Context<InitializeMasterPool>,
    admin_multisig: Pubkey,
    timelock_authority: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.version = 1;
    config.is_initialized = true;
    config.claw_mint = ctx.accounts.claw_mint.key();
    config.master_pool_vault = Pubkey::default();
    config.treasury_vault = Pubkey::default();
    config.compute_pool_bps = COMPUTE_POOL_BPS;
    config.outcome_pool_bps = OUTCOME_POOL_BPS;
    config.treasury_tax_bps = TREASURY_TAX_BPS;
    config.genesis_total_supply = GENESIS_TOTAL_SUPPLY;
    config.genesis_minted = false;
    config.mint_authority_revoked = false;
    config.freeze_authority_revoked = false;
    config.upgrade_frozen = false;
    config.admin_multisig = admin_multisig;
    config.timelock_authority = timelock_authority;
    config.current_epoch = 0;
    config.settlement_enabled = false;
    config.deployer = ctx.accounts.deployer.key();
    config.created_at = Clock::get()?.unix_timestamp;
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!("ClawFarm Master Pool initialized");
    Ok(())
}

pub fn create_master_pool_vault(ctx: Context<CreateMasterPoolVault>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.master_pool_vault = ctx.accounts.master_pool_vault.key();
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!(
        "Master Pool Vault created: {}",
        ctx.accounts.master_pool_vault.key()
    );
    Ok(())
}

pub fn create_treasury_vault(ctx: Context<CreateTreasuryVault>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.treasury_vault = ctx.accounts.treasury_vault.key();
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!(
        "Treasury Vault created: {}",
        ctx.accounts.treasury_vault.key()
    );
    Ok(())
}

pub fn mint_genesis_supply(ctx: Context<MintGenesisSupply>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.genesis_minted, ErrorCode::GenesisAlreadyMinted);

    let bump = ctx.bumps.pool_authority;
    let seeds = &[POOL_AUTHORITY_SEED, &[bump]];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts = MintTo {
        mint: ctx.accounts.claw_mint.to_account_info(),
        to: ctx.accounts.master_pool_vault.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    token::mint_to(cpi_ctx, GENESIS_TOTAL_SUPPLY)?;

    config.genesis_minted = true;
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!("Genesis supply minted: {} CLAW", GENESIS_TOTAL_SUPPLY);
    Ok(())
}

pub fn revoke_mint_authority(ctx: Context<RevokeMintAuthority>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(config.genesis_minted, ErrorCode::GenesisNotMinted);
    require!(!config.mint_authority_revoked, ErrorCode::AlreadyRevoked);

    let bump = ctx.bumps.pool_authority;
    let seeds = &[POOL_AUTHORITY_SEED, &[bump]];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts = SetAuthority {
        current_authority: ctx.accounts.pool_authority.to_account_info(),
        account_or_mint: ctx.accounts.claw_mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    token::set_authority(cpi_ctx, AuthorityType::MintTokens, None)?;

    config.mint_authority_revoked = true;
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!("Mint authority permanently revoked");
    Ok(())
}

pub fn revoke_freeze_authority(ctx: Context<RevokeFreezeAuthority>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.freeze_authority_revoked, ErrorCode::AlreadyRevoked);

    let bump = ctx.bumps.pool_authority;
    let seeds = &[POOL_AUTHORITY_SEED, &[bump]];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts = SetAuthority {
        current_authority: ctx.accounts.pool_authority.to_account_info(),
        account_or_mint: ctx.accounts.claw_mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    token::set_authority(cpi_ctx, AuthorityType::FreezeAccount, None)?;

    config.freeze_authority_revoked = true;
    config.updated_at = Clock::get()?.unix_timestamp;

    msg!("Freeze authority permanently revoked");
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeMasterPool<'info> {
    #[account(
        init,
        payer = deployer,
        space = 8 + std::mem::size_of::<ClawFarmConfig>(),
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    pub claw_mint: Account<'info, Mint>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateMasterPoolVault<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.deployer == deployer.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    #[account(
        init,
        payer = deployer,
        token::mint = claw_mint,
        token::authority = pool_authority,
        seeds = [MASTER_POOL_VAULT_SEED],
        bump,
    )]
    pub master_pool_vault: Account<'info, TokenAccount>,
    /// CHECK: PDA authority for the pool
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub claw_mint: Account<'info, Mint>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateTreasuryVault<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.deployer == deployer.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    #[account(
        init,
        payer = deployer,
        token::mint = usdc_mint,
        token::authority = pool_authority,
        seeds = [TREASURY_VAULT_SEED],
        bump,
    )]
    pub treasury_vault: Account<'info, TokenAccount>,
    /// CHECK: PDA authority for the pool
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub usdc_mint: Account<'info, Mint>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintGenesisSupply<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.deployer == deployer.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    #[account(
        mut,
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [MASTER_POOL_VAULT_SEED],
        bump,
        constraint = master_pool_vault.key() == config.master_pool_vault @ ErrorCode::InvalidVault,
    )]
    pub master_pool_vault: Account<'info, TokenAccount>,
    /// CHECK: PDA authority — mint authority must be this PDA
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RevokeMintAuthority<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.deployer == deployer.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    #[account(
        mut,
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    /// CHECK: PDA authority
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RevokeFreezeAuthority<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.deployer == deployer.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,
    #[account(
        mut,
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    /// CHECK: PDA authority
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
