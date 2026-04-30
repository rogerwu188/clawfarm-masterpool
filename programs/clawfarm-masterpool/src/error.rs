use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The admin authority is not authorized for this action")]
    UnauthorizedAdmin,
    #[msg("The configured attestation caller is invalid")]
    UnauthorizedAttestationCaller,
    #[msg("The configured attestation program is invalid")]
    InvalidAttestationProgram,
    #[msg("Program data account does not match this program")]
    InvalidProgramData,
    #[msg("Initializer is not the current program upgrade authority")]
    UnauthorizedInitializer,
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
    #[msg("The payment delegate is not approved for the payer token account")]
    InvalidPaymentDelegate,
    #[msg("The payment delegate allowance is below the receipt charge")]
    InsufficientDelegatedAllowance,
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
    #[msg("The CLAW mint authority must be the pool authority before genesis minting")]
    InvalidClawMintAuthority,
    #[msg("The CLAW freeze authority must be the pool authority before genesis minting")]
    InvalidClawFreezeAuthority,
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
    #[msg("The receipt charge exceeds the supported Phase 1 settlement domain")]
    ReceiptChargeTooLarge,
    #[msg("No released CLAW is available to claim")]
    NoClaimableRewards,
    #[msg("The reward release amount exceeds the locked balance")]
    RewardReleaseExceedsLocked,
    #[msg("The requested reward release exceeds the currently vested amount")]
    RewardReleaseExceedsVested,
    #[msg("The receipt lock period has not started")]
    RewardLockNotStarted,
    #[msg("The faucet is disabled")]
    FaucetDisabled,
    #[msg("The faucet claim amount is invalid")]
    InvalidFaucetAmount,
    #[msg("The faucet limits are invalid")]
    InvalidFaucetLimits,
    #[msg("The faucet per-claim limit was exceeded")]
    FaucetClaimLimitExceeded,
    #[msg("The faucet wallet daily limit was exceeded")]
    FaucetWalletDailyLimitExceeded,
    #[msg("The faucet global daily limit was exceeded")]
    FaucetGlobalDailyLimitExceeded,
    #[msg("The faucet vault balance is insufficient")]
    FaucetVaultInsufficientBalance,
    #[msg("The faucet vault account is invalid")]
    InvalidFaucetVault,
    #[msg("The faucet user state account is invalid")]
    InvalidFaucetUserState,
}
