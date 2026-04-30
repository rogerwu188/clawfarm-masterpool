use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub authority: Pubkey,
    pub pause_authority: Pubkey,
    pub challenge_resolver: Pubkey,
    pub masterpool_program: Pubkey,
}

#[event]
pub struct ProviderSignerUpserted {
    pub signer: Pubkey,
    pub provider_wallet: Pubkey,
    pub attester_type_mask: u8,
}

#[event]
pub struct ProviderSignerRevoked {
    pub signer: Pubkey,
    pub provider_wallet: Pubkey,
}

#[event]
pub struct PauseUpdated {
    pub is_paused: bool,
}

#[event]
pub struct ReceiptSubmitted {
    pub receipt: Pubkey,
    pub request_nonce_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub provider_wallet: Pubkey,
    pub payer_user: Pubkey,
    pub signer: Pubkey,
    pub receipt_hash: [u8; 32],
    pub challenge_deadline: i64,
}

#[event]
pub struct ReceiptFinalized {
    pub receipt: Pubkey,
    pub signer: Pubkey,
    pub receipt_hash: [u8; 32],
}

#[event]
pub struct ReceiptClosed {
    pub receipt: Pubkey,
    pub signer: Pubkey,
    pub receipt_hash: [u8; 32],
    pub status: u8,
}

#[event]
pub struct ChallengeOpened {
    pub challenge: Pubkey,
    pub receipt: Pubkey,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub bond_amount: u64,
}

#[event]
pub struct ChallengeResolved {
    pub challenge: Pubkey,
    pub receipt: Pubkey,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub resolution_code: u8,
}

#[event]
pub struct ChallengeClosed {
    pub challenge: Pubkey,
    pub receipt: Pubkey,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub resolution_code: u8,
}
