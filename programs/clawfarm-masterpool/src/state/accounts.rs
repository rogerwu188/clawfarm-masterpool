use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    pub version: u8,
    pub admin_authority: Pubkey,
    pub attestation_program: Pubkey,
    pub claw_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub reward_vault: Pubkey,
    pub challenge_bond_vault: Pubkey,
    pub treasury_usdc_vault: Pubkey,
    pub provider_stake_usdc_vault: Pubkey,
    pub provider_pending_usdc_vault: Pubkey,
    pub exchange_rate_claw_per_usdc_e6: u64,
    pub provider_stake_usdc: u64,
    pub provider_usdc_share_bps: u16,
    pub treasury_usdc_share_bps: u16,
    pub user_claw_share_bps: u16,
    pub provider_claw_share_bps: u16,
    pub lock_days: u16,
    pub provider_slash_claw_amount: u64,
    pub challenger_reward_bps: u16,
    pub burn_bps: u16,
    pub challenge_bond_claw_amount: u64,
    pub genesis_minted: bool,
    pub pause_receipt_recording: bool,
    pub pause_challenge_processing: bool,
    pub pause_finalization: bool,
    pub pause_claims: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct ProviderAccount {
    pub provider_wallet: Pubkey,
    pub staked_usdc_amount: u64,
    pub pending_provider_usdc: u64,
    pub claw_net_position: i64,
    pub unsettled_receipt_count: u64,
    pub unresolved_challenge_count: u64,
    pub status: u8,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct RewardAccount {
    pub initialized: bool,
    pub owner: Pubkey,
    pub reward_kind: u8,
    pub locked_claw_total: u64,
    pub released_claw_total: u64,
    pub claimed_claw_total: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct ReceiptSettlement {
    pub attestation_receipt: Pubkey,
    pub payer_user: Pubkey,
    pub provider_wallet: Pubkey,
    pub usdc_total_paid: u64,
    pub usdc_to_provider: u64,
    pub usdc_to_treasury: u64,
    pub claw_to_user: u64,
    pub claw_to_provider_total: u64,
    pub claw_provider_debt_offset: u64,
    pub claw_to_provider_locked: u64,
    pub status: u8,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct ChallengeBondRecord {
    pub attestation_challenge: Pubkey,
    pub attestation_receipt: Pubkey,
    pub challenger: Pubkey,
    pub payer_user: Pubkey,
    pub provider_wallet: Pubkey,
    pub bond_amount: u64,
    pub slash_claw_amount_snapshot: u64,
    pub challenger_reward_bps_snapshot: u16,
    pub burn_bps_snapshot: u16,
    pub challenger_reward_amount: u64,
    pub burn_amount: u64,
    pub status: u8,
    pub created_at: i64,
    pub updated_at: i64,
}
