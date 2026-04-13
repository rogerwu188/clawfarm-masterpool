use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The admin authority is not authorized for this action")]
    UnauthorizedAdmin,
    #[msg("The configured attestation caller is invalid")]
    UnauthorizedAttestationCaller,
    #[msg("The configured attestation program is invalid")]
    InvalidAttestationProgram,
    #[msg("The CLAW mint is invalid")]
    InvalidClawMint,
    #[msg("The USDC mint is invalid")]
    InvalidUsdcMint,
    #[msg("One of the configured vault accounts is invalid")]
    InvalidVaultAccount,
    #[msg("The token account owner is invalid")]
    InvalidTokenOwner,
    #[msg("The token account mint is invalid")]
    InvalidTokenMint,
    #[msg("The reward account owner is invalid")]
    InvalidRewardAccountOwner,
    #[msg("The reward account kind is invalid")]
    InvalidRewardAccountKind,
    #[msg("The provider account is invalid")]
    InvalidProviderAccount,
    #[msg("The receipt settlement account is invalid")]
    InvalidReceiptSettlement,
    #[msg("The challenge bond record is invalid")]
    InvalidChallengeBondRecord,
    #[msg("The supplied mint decimals do not match the Phase 1 assumptions")]
    InvalidMintDecimals,
    #[msg("The supplied governance parameters are invalid")]
    InvalidGovernanceParameters,
    #[msg("Split ratios must sum to 1000")]
    InvalidSplitInvariant,
    #[msg("The supplied amount must be positive")]
    InvalidPositiveAmount,
    #[msg("A numeric calculation overflowed")]
    MathOverflow,
    #[msg("The one-time genesis mint has already been executed")]
    GenesisAlreadyMinted,
    #[msg("Receipt recording is paused")]
    ReceiptRecordingPaused,
    #[msg("Challenge processing is paused")]
    ChallengeProcessingPaused,
    #[msg("Receipt finalization is paused")]
    FinalizationPaused,
    #[msg("Reward claims are paused")]
    ClaimsPaused,
    #[msg("The provider is already registered")]
    ProviderAlreadyRegistered,
    #[msg("The provider is not active")]
    ProviderNotActive,
    #[msg("Provider exit is blocked by unresolved obligations")]
    ProviderExitBlocked,
    #[msg("A receipt settlement for this attestation receipt already exists")]
    ReceiptSettlementAlreadyExists,
    #[msg("The receipt settlement is not in the required state")]
    InvalidReceiptSettlementState,
    #[msg("A challenge bond record for this attestation challenge already exists")]
    ChallengeBondAlreadyExists,
    #[msg("The challenge bond record is not in the required state")]
    InvalidChallengeBondState,
    #[msg("The attestation receipt status is invalid for this operation")]
    InvalidAttestationReceiptStatus,
    #[msg("The attestation challenge resolution is invalid for this operation")]
    InvalidChallengeResolution,
    #[msg("The payer's charge mint does not match the configured USDC mint")]
    ChargeMintMismatch,
    #[msg("No released CLAW is available to claim")]
    NoClaimableRewards,
    #[msg("The reward release amount exceeds the locked balance")]
    RewardReleaseExceedsLocked,
}
