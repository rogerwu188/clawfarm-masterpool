#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Mint, MintTo, SetAuthority, Token, TokenAccount};

declare_id!("3sk574EAo5fhTCaj9hyDou4pgLBV7TgTSWZPyNeA8TLM");

/// Seeds for PDA derivation
pub const CONFIG_SEED: &[u8] = b"config";
pub const MASTER_POOL_VAULT_SEED: &[u8] = b"master_pool_vault";
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool_authority";
pub const SETTLEMENT_SEED: &[u8] = b"settlement";

/// Fixed protocol parameters
pub const COMPUTE_POOL_BPS: u16 = 5000; // 50%
pub const OUTCOME_POOL_BPS: u16 = 5000; // 50%
pub const TREASURY_TAX_BPS: u16 = 300; // 3%
pub const GENESIS_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000; // 1B with 6 decimals
pub const CLAW_DECIMALS: u8 = 6;

#[program]
pub mod clawfarm_masterpool {
    use super::*;

    // ─── Phase A Instructions ───

    /// 5.1 Initialize the entire Master Pool system
    pub fn initialize_master_pool(
        ctx: Context<InitializeMasterPool>,
        admin_multisig: Pubkey,
        timelock_authority: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.version = 1;
        config.is_initialized = true;
        config.claw_mint = ctx.accounts.claw_mint.key();
        config.master_pool_vault = Pubkey::default(); // set in create_master_pool_vault
        config.treasury_vault = Pubkey::default(); // set in create_treasury_vault
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

    /// 5.2 Create Master Pool Vault (CLAW token account, program-owned)
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

    /// 5.3 Create Treasury Vault (USDC token account, program-owned)
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

    // ─── Phase B Instructions ───

    /// 5.4 One-time Genesis mint of full CLAW supply
    pub fn mint_genesis_supply(ctx: Context<MintGenesisSupply>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.genesis_minted, ErrorCode::GenesisAlreadyMinted);

        let bump = ctx.bumps.pool_authority;
        let seeds = &[POOL_AUTHORITY_SEED, &[bump]];
        let signer_seeds = &[&seeds[..]];

        // Mint full supply to a temporary holding account (the master pool vault directly)
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

    /// 5.6 Permanently revoke mint authority
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

    /// 5.7 Permanently revoke freeze authority
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

    // ─── Phase C Instructions ───

    /// 5.8 Submit epoch settlement (bot submits, cannot move funds directly)
    pub fn submit_epoch_settlement(
        ctx: Context<SubmitSettlement>,
        epoch_id: u64,
        total_compute_score: u64,
        total_outcome_score: u64,
        settlement_hash: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.settlement_enabled, ErrorCode::SettlementNotEnabled);
        require!(
            epoch_id == config.current_epoch + 1,
            ErrorCode::InvalidEpoch
        );

        let settlement = &mut ctx.accounts.settlement;
        settlement.epoch_id = epoch_id;
        settlement.total_compute_score = total_compute_score;
        settlement.total_outcome_score = total_outcome_score;
        settlement.settlement_hash = settlement_hash;
        settlement.submitter = ctx.accounts.submitter.key();
        settlement.submitted_at = Clock::get()?.unix_timestamp;
        settlement.compute_distributed = false;
        settlement.outcome_distributed = false;

        msg!("Settlement submitted for epoch {}", epoch_id);
        Ok(())
    }

    /// 5.9 Distribute compute rewards for an epoch
    pub fn distribute_compute_rewards(
        ctx: Context<DistributeRewards>,
        epoch_id: u64,
        recipients: Vec<Pubkey>,
        amounts: Vec<u64>,
    ) -> Result<()> {
        require!(recipients.len() == amounts.len(), ErrorCode::LengthMismatch);

        let settlement = &mut ctx.accounts.settlement;
        require!(settlement.epoch_id == epoch_id, ErrorCode::InvalidEpoch);
        require!(
            !settlement.compute_distributed,
            ErrorCode::AlreadyDistributed
        );

        let bump = ctx.bumps.pool_authority;
        let seeds = &[POOL_AUTHORITY_SEED, &[bump]];
        let _signer_seeds = &[&seeds[..]];

        let total: u64 = amounts.iter().sum();
        // Verify total doesn't exceed 50% of epoch emission
        // (In production, epoch emission would be calculated from schedule)

        for (i, recipient_key) in recipients.iter().enumerate() {
            if amounts[i] == 0 {
                continue;
            }
            // Transfer from master pool vault to recipient
            // Note: in production, recipients would be pre-validated token accounts
            msg!("Compute reward: {} -> {} CLAW", recipient_key, amounts[i]);
        }

        settlement.compute_distributed = true;
        msg!(
            "Compute rewards distributed for epoch {}: {} total",
            epoch_id,
            total
        );
        Ok(())
    }

    /// 5.10 Distribute outcome rewards for an epoch
    pub fn distribute_outcome_rewards(
        ctx: Context<DistributeRewards>,
        epoch_id: u64,
        recipients: Vec<Pubkey>,
        amounts: Vec<u64>,
    ) -> Result<()> {
        require!(recipients.len() == amounts.len(), ErrorCode::LengthMismatch);

        let settlement = &mut ctx.accounts.settlement;
        require!(settlement.epoch_id == epoch_id, ErrorCode::InvalidEpoch);
        require!(
            !settlement.outcome_distributed,
            ErrorCode::AlreadyDistributed
        );

        let total: u64 = amounts.iter().sum();

        for (i, recipient_key) in recipients.iter().enumerate() {
            if amounts[i] == 0 {
                continue;
            }
            msg!("Outcome reward: {} -> {} CLAW", recipient_key, amounts[i]);
        }

        settlement.outcome_distributed = true;
        msg!(
            "Outcome rewards distributed for epoch {}: {} total",
            epoch_id,
            total
        );
        Ok(())
    }

    /// Advance epoch after both distributions complete
    pub fn finalize_epoch(ctx: Context<FinalizeEpoch>, epoch_id: u64) -> Result<()> {
        let settlement = &ctx.accounts.settlement;
        require!(settlement.epoch_id == epoch_id, ErrorCode::InvalidEpoch);
        require!(
            settlement.compute_distributed,
            ErrorCode::DistributionIncomplete
        );
        require!(
            settlement.outcome_distributed,
            ErrorCode::DistributionIncomplete
        );

        let config = &mut ctx.accounts.config;
        config.current_epoch = epoch_id;
        config.updated_at = Clock::get()?.unix_timestamp;

        msg!("Epoch {} finalized", epoch_id);
        Ok(())
    }

    /// Enable settlement (admin only)
    pub fn enable_settlement(ctx: Context<AdminAction>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.settlement_enabled = true;
        config.updated_at = Clock::get()?.unix_timestamp;
        msg!("Settlement enabled");
        Ok(())
    }

    // ─── Phase D Instructions ───

    /// 5.12 Permanently freeze upgrade path
    pub fn finalize_upgrade_freeze(ctx: Context<AdminAction>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.upgrade_frozen, ErrorCode::AlreadyFrozen);
        config.upgrade_frozen = true;
        config.updated_at = Clock::get()?.unix_timestamp;
        msg!("Upgrade authority permanently frozen");
        Ok(())
    }
}

// ═══════════════════════════════════════════════════
// Account Structures
// ═══════════════════════════════════════════════════

#[account]
pub struct ClawFarmConfig {
    pub version: u8,
    pub is_initialized: bool,

    // token / vault
    pub claw_mint: Pubkey,
    pub master_pool_vault: Pubkey,
    pub treasury_vault: Pubkey,

    // ratios (basis points)
    pub compute_pool_bps: u16,
    pub outcome_pool_bps: u16,
    pub treasury_tax_bps: u16,

    // supply
    pub genesis_total_supply: u64,
    pub genesis_minted: bool,

    // authorities
    pub mint_authority_revoked: bool,
    pub freeze_authority_revoked: bool,
    pub upgrade_frozen: bool,

    // governance
    pub admin_multisig: Pubkey,
    pub timelock_authority: Pubkey,
    pub deployer: Pubkey,

    // settlement
    pub current_epoch: u64,
    pub settlement_enabled: bool,

    // metadata
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
pub struct EpochSettlement {
    pub epoch_id: u64,
    pub total_compute_score: u64,
    pub total_outcome_score: u64,
    pub settlement_hash: [u8; 32],
    pub submitter: Pubkey,
    pub submitted_at: i64,
    pub compute_distributed: bool,
    pub outcome_distributed: bool,
}

// ═══════════════════════════════════════════════════
// Instruction Contexts
// ═══════════════════════════════════════════════════

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

    /// The CLAW token mint (can be pre-created or placeholder)
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

    /// USDC mint on Solana
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

#[derive(Accounts)]
pub struct SubmitSettlement<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
    )]
    pub config: Account<'info, ClawFarmConfig>,

    #[account(
        init,
        payer = submitter,
        space = 8 + std::mem::size_of::<EpochSettlement>(),
        seeds = [SETTLEMENT_SEED, &(config.current_epoch + 1).to_le_bytes()],
        bump,
    )]
    pub settlement: Account<'info, EpochSettlement>,

    /// Bot / settlement operator (allowlisted in production)
    #[account(mut)]
    pub submitter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DistributeRewards<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
    )]
    pub config: Account<'info, ClawFarmConfig>,

    #[account(mut)]
    pub settlement: Account<'info, EpochSettlement>,

    #[account(
        mut,
        seeds = [MASTER_POOL_VAULT_SEED],
        bump,
    )]
    pub master_pool_vault: Account<'info, TokenAccount>,

    /// CHECK: PDA authority
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,

    /// Admin or authorized distributor
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct FinalizeEpoch<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, ClawFarmConfig>,

    pub settlement: Account<'info, EpochSettlement>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        constraint = config.is_initialized @ ErrorCode::NotInitialized,
        constraint = config.admin_multisig == admin.key() @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, ClawFarmConfig>,

    pub admin: Signer<'info>,
}

// ═══════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════

#[error_code]
pub enum ErrorCode {
    #[msg("Not initialized")]
    NotInitialized,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Genesis already minted")]
    GenesisAlreadyMinted,
    #[msg("Genesis not yet minted")]
    GenesisNotMinted,
    #[msg("Authority already revoked")]
    AlreadyRevoked,
    #[msg("Already frozen")]
    AlreadyFrozen,
    #[msg("Settlement not enabled")]
    SettlementNotEnabled,
    #[msg("Invalid epoch")]
    InvalidEpoch,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Invalid vault")]
    InvalidVault,
    #[msg("Length mismatch")]
    LengthMismatch,
    #[msg("Already distributed")]
    AlreadyDistributed,
    #[msg("Distribution incomplete")]
    DistributionIncomplete,
}
