use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Not initialized")]
    NotInitialized,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Genesis already minted")]
    GenesisAlreadyMinted,
    #[msg("Genesis not yet minted")]
    GenesisNotMinted,
    #[msg("Authority already revoked")]
    AlreadyRevoked,
    #[msg("Already frozen")]
    AlreadyFrozen,
    #[msg("Settlement not enabled")]
    SettlementNotEnabled,
    #[msg("Invalid epoch")]
    InvalidEpoch,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Invalid vault")]
    InvalidVault,
    #[msg("Length mismatch")]
    LengthMismatch,
    #[msg("Already distributed")]
    AlreadyDistributed,
    #[msg("Distribution incomplete")]
    DistributionIncomplete,
}
