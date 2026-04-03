use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Invalid attestation version")]
    InvalidVersion,
    #[msg("Invalid proof mode")]
    InvalidProofMode,
    #[msg("Invalid request nonce")]
    InvalidRequestNonce,
    #[msg("Invalid proof id")]
    InvalidProofId,
    #[msg("Invalid provider code")]
    InvalidProvider,
    #[msg("Invalid model")]
    InvalidModel,
    #[msg("Invalid usage basis")]
    InvalidUsageBasis,
    #[msg("Invalid attester type")]
    InvalidAttesterType,
    #[msg("Invalid token totals")]
    InvalidTokenTotals,
    #[msg("HTTP status is invalid")]
    InvalidHttpStatus,
    #[msg("Receipt is expired or has inconsistent timestamps")]
    ReceiptExpired,
    #[msg("Signer is inactive")]
    SignerInactive,
    #[msg("Signer is not yet valid")]
    SignerNotYetValid,
    #[msg("Signer validity has expired")]
    SignerExpired,
    #[msg("Signer does not match the registry")]
    SignerMismatch,
    #[msg("Provider does not match the registry")]
    ProviderMismatch,
    #[msg("Signer is not authorized for the requested attester type")]
    SignerAttesterTypeMismatch,
    #[msg("The challenge window is closed")]
    ChallengeWindowClosed,
    #[msg("The response window is closed")]
    ResponseWindowClosed,
    #[msg("Receipt is not challengeable")]
    ReceiptNotChallengeable,
    #[msg("Receipt nonce does not match the receipt account")]
    ReceiptNonceMismatch,
    #[msg("Challenge is not open")]
    ChallengeNotOpen,
    #[msg("Responder is not authorized for this challenge")]
    ChallengeResponderUnauthorized,
    #[msg("Challenge cannot be resolved in its current state")]
    ChallengeNotResolvable,
    #[msg("Challenge resolution is invalid")]
    ChallengeResolutionInvalid,
    #[msg("Challenge type is invalid")]
    ChallengeTypeInvalid,
    #[msg("Challenge type does not match the challenge account")]
    ChallengeTypeMismatch,
    #[msg("Challenge challenger does not match the challenge account")]
    ChallengeChallengerMismatch,
    #[msg("Receipt is not finalizable")]
    ReceiptNotFinalizable,
    #[msg("Challenge window is still open")]
    ChallengeWindowOpen,
    #[msg("Attester type mask must not be zero")]
    InvalidAttesterTypeMask,
    #[msg("Signer validity window is invalid")]
    InvalidValidityWindow,
    #[msg("Window value is invalid")]
    InvalidWindow,
    #[msg("String exceeds the phase 1 maximum length")]
    StringTooLong,
    #[msg("Proof URL is invalid")]
    InvalidProofUrl,
    #[msg("Receipt hash does not match the canonical payload")]
    ReceiptHashMismatch,
    #[msg("Matching ed25519 verification instruction is missing")]
    MissingEd25519Instruction,
    #[msg("ed25519 verification instruction does not match the receipt args")]
    Ed25519InstructionMismatch,
    #[msg("Receipt is not in a terminal state and cannot be closed")]
    ReceiptNotClosable,
    #[msg("Challenge is not in a terminal state and cannot be closed")]
    ChallengeNotClosable,
}
