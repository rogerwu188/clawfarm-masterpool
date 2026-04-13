#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

mod constants;
mod error;
mod instructions;
mod state;

pub use constants::*;
pub use error::ErrorCode;
use instructions::admin::__client_accounts_admin_action;
use instructions::distribution::{
    __client_accounts_distribute_rewards, __client_accounts_finalize_epoch,
    __client_accounts_submit_settlement,
};
use instructions::setup::{
    __client_accounts_create_master_pool_vault, __client_accounts_create_treasury_vault,
    __client_accounts_initialize_master_pool, __client_accounts_mint_genesis_supply,
    __client_accounts_revoke_freeze_authority, __client_accounts_revoke_mint_authority,
};
pub use instructions::{
    AdminAction, CreateMasterPoolVault, CreateTreasuryVault, DistributeRewards, FinalizeEpoch,
    InitializeMasterPool, MintGenesisSupply, RevokeFreezeAuthority, RevokeMintAuthority,
    SubmitSettlement,
};
pub use state::*;

declare_id!("3sk574EAo5fhTCaj9hyDou4pgLBV7TgTSWZPyNeA8TLM");

#[program]
pub mod clawfarm_masterpool {
    use super::*;

    pub fn initialize_master_pool(
        ctx: Context<InitializeMasterPool>,
        admin_multisig: Pubkey,
        timelock_authority: Pubkey,
    ) -> Result<()> {
        instructions::setup::initialize_master_pool(ctx, admin_multisig, timelock_authority)
    }

    pub fn create_master_pool_vault(ctx: Context<CreateMasterPoolVault>) -> Result<()> {
        instructions::setup::create_master_pool_vault(ctx)
    }

    pub fn create_treasury_vault(ctx: Context<CreateTreasuryVault>) -> Result<()> {
        instructions::setup::create_treasury_vault(ctx)
    }

    pub fn mint_genesis_supply(ctx: Context<MintGenesisSupply>) -> Result<()> {
        instructions::setup::mint_genesis_supply(ctx)
    }

    pub fn revoke_mint_authority(ctx: Context<RevokeMintAuthority>) -> Result<()> {
        instructions::setup::revoke_mint_authority(ctx)
    }

    pub fn revoke_freeze_authority(ctx: Context<RevokeFreezeAuthority>) -> Result<()> {
        instructions::setup::revoke_freeze_authority(ctx)
    }

    pub fn submit_epoch_settlement(
        ctx: Context<SubmitSettlement>,
        epoch_id: u64,
        total_compute_score: u64,
        total_outcome_score: u64,
        settlement_hash: [u8; 32],
    ) -> Result<()> {
        instructions::distribution::submit_epoch_settlement(
            ctx,
            epoch_id,
            total_compute_score,
            total_outcome_score,
            settlement_hash,
        )
    }

    pub fn distribute_compute_rewards(
        ctx: Context<DistributeRewards>,
        epoch_id: u64,
        recipients: Vec<Pubkey>,
        amounts: Vec<u64>,
    ) -> Result<()> {
        instructions::distribution::distribute_compute_rewards(ctx, epoch_id, recipients, amounts)
    }

    pub fn distribute_outcome_rewards(
        ctx: Context<DistributeRewards>,
        epoch_id: u64,
        recipients: Vec<Pubkey>,
        amounts: Vec<u64>,
    ) -> Result<()> {
        instructions::distribution::distribute_outcome_rewards(ctx, epoch_id, recipients, amounts)
    }

    pub fn finalize_epoch(ctx: Context<FinalizeEpoch>, epoch_id: u64) -> Result<()> {
        instructions::distribution::finalize_epoch(ctx, epoch_id)
    }

    pub fn enable_settlement(ctx: Context<AdminAction>) -> Result<()> {
        instructions::admin::enable_settlement(ctx)
    }

    pub fn finalize_upgrade_freeze(ctx: Context<AdminAction>) -> Result<()> {
        instructions::admin::finalize_upgrade_freeze(ctx)
    }
}
