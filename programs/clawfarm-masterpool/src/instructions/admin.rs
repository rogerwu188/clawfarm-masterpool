use anchor_lang::prelude::*;

use crate::{constants::CONFIG_SEED, error::ErrorCode, state::ClawFarmConfig};

pub fn enable_settlement(ctx: Context<AdminAction>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.settlement_enabled = true;
    config.updated_at = Clock::get()?.unix_timestamp;
    msg!("Settlement enabled");
    Ok(())
}

pub fn finalize_upgrade_freeze(ctx: Context<AdminAction>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.upgrade_frozen, ErrorCode::AlreadyFrozen);
    config.upgrade_frozen = true;
    config.updated_at = Clock::get()?.unix_timestamp;
    msg!("Upgrade authority permanently frozen");
    Ok(())
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
