use anchor_lang::prelude::*;

#[account]
pub struct ClawFarmConfig {
    pub version: u8,
    pub is_initialized: bool,
    pub claw_mint: Pubkey,
    pub master_pool_vault: Pubkey,
    pub treasury_vault: Pubkey,
    pub compute_pool_bps: u16,
    pub outcome_pool_bps: u16,
    pub treasury_tax_bps: u16,
    pub genesis_total_supply: u64,
    pub genesis_minted: bool,
    pub mint_authority_revoked: bool,
    pub freeze_authority_revoked: bool,
    pub upgrade_frozen: bool,
    pub admin_multisig: Pubkey,
    pub timelock_authority: Pubkey,
    pub deployer: Pubkey,
    pub current_epoch: u64,
    pub settlement_enabled: bool,
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
