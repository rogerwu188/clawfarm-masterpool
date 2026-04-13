use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub pause_authority: Pubkey,
    pub challenge_resolver: Pubkey,
    pub masterpool_program: Pubkey,
    pub challenge_window_seconds: i64,
    pub is_paused: bool,
}

#[account]
#[derive(InitSpace)]
pub struct ProviderSigner {
    pub attester_type_mask: u8,
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
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
    pub economics_settled: bool,
}

#[account]
#[derive(InitSpace)]
pub struct Challenge {
    pub receipt: Pubkey,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub evidence_hash: [u8; 32],
    pub bond_amount: u64,
    pub opened_at: i64,
    pub resolved_at: i64,
    pub status: u8,
    pub resolution_code: u8,
}
