use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

use crate::{
    constants::{CONFIG_SEED, MASTER_POOL_VAULT_SEED, POOL_AUTHORITY_SEED, SETTLEMENT_SEED},
    error::ErrorCode,
    state::{ClawFarmConfig, EpochSettlement},
};

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

    for (i, recipient_key) in recipients.iter().enumerate() {
        if amounts[i] == 0 {
            continue;
        }
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
