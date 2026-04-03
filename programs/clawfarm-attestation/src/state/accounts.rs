use anchor_lang::prelude::*;

use crate::constants::{MAX_KEY_ID_LEN, MAX_PROVIDER_LEN};

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub pause_authority: Pubkey,
    pub challenge_resolver: Pubkey,
    pub challenge_window_seconds: i64,
    pub response_window_seconds: i64,
    pub receipt_count: u64,
    pub challenge_count: u64,
    pub is_paused: bool,
    pub phase2_enabled: bool,
    pub bump: u8,
    pub reserved: [u8; 32],
}

#[account]
#[derive(InitSpace)]
pub struct ProviderSigner {
    #[max_len(MAX_PROVIDER_LEN)]
    pub provider_code: String,
    pub signer: Pubkey,
    #[max_len(MAX_KEY_ID_LEN)]
    pub key_id: String,
    pub attester_type_mask: u8,
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
    pub metadata_hash: [u8; 32],
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
    pub reserved: [u8; 32],
}

#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
    pub submitted_at: i64,
    pub challenge_deadline: i64,
    pub finalized_at: i64,
    pub status: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Challenge {
    pub receipt: Pubkey,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub evidence_hash: [u8; 32],
    pub response_hash: [u8; 32],
    pub opened_at: i64,
    pub response_deadline: i64,
    pub resolved_at: i64,
    pub status: u8,
    pub resolution_code: u8,
    pub bump: u8,
}
