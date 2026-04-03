use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub authority: Pubkey,
    pub pause_authority: Pubkey,
    pub challenge_resolver: Pubkey,
}

#[event]
pub struct ProviderSignerUpserted {
    pub provider_code: String,
    pub signer: Pubkey,
    pub attester_type_mask: u8,
}

#[event]
pub struct ProviderSignerRevoked {
    pub provider_code: String,
    pub signer: Pubkey,
}

#[event]
pub struct PauseUpdated {
    pub is_paused: bool,
}

#[event]
pub struct ReceiptSubmitted {
    pub receipt: Pubkey,
    pub request_nonce: String,
    pub proof_id: String,
    pub provider: String,
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
