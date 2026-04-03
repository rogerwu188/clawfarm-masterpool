use anchor_lang::prelude::*;

use crate::error::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SubmitReceiptArgs {
    pub version: u8,
    pub proof_mode: u8,
    pub proof_id: String,
    pub request_nonce: String,
    pub provider: String,
    pub attester_type: u8,
    pub model: String,
    pub usage_basis: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub charge_atomic: u64,
    pub charge_mint: Pubkey,
    pub provider_request_id: Option<String>,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
}

#[repr(u8)]
pub enum ProofMode {
    SigLog = 0,
    SigLogZkReserved = 1,
}

#[repr(u8)]
pub enum AttesterType {
    Provider = 0,
    Gateway = 1,
    Hybrid = 2,
}

#[repr(u8)]
pub enum UsageBasis {
    ProviderReported = 0,
    ServerEstimatedReserved = 1,
    HybridReserved = 2,
    TokenizerVerifiedReserved = 3,
}

#[repr(u8)]
pub enum SignerStatus {
    Inactive = 0,
    Active = 1,
    Revoked = 2,
}

#[repr(u8)]
pub enum ReceiptStatus {
    Submitted = 0,
    Challenged = 1,
    Finalized = 2,
    Rejected = 3,
    Slashed = 4,
}

#[repr(u8)]
pub enum ChallengeStatus {
    Open = 0,
    Accepted = 1,
    Rejected = 2,
    Expired = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChallengeType {
    InvalidSignature = 0,
    SignerRegistryMismatch = 1,
    ReplayNonce = 2,
    InvalidLogInclusion = 3,
    PayloadMismatch = 4,
}

impl TryFrom<u8> for ChallengeType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::InvalidSignature),
            1 => Ok(Self::SignerRegistryMismatch),
            2 => Ok(Self::ReplayNonce),
            3 => Ok(Self::InvalidLogInclusion),
            4 => Ok(Self::PayloadMismatch),
            _ => err!(ErrorCode::ChallengeTypeInvalid),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ResolutionCode {
    None = 0,
    Accepted = 1,
    Rejected = 2,
    ReceiptInvalidated = 3,
    SignerRevoked = 4,
}

impl TryFrom<u8> for ResolutionCode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Accepted),
            2 => Ok(Self::Rejected),
            3 => Ok(Self::ReceiptInvalidated),
            4 => Ok(Self::SignerRevoked),
            _ => err!(ErrorCode::ChallengeResolutionInvalid),
        }
    }
}
