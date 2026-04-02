#![allow(unexpected_cfgs)]

// Phase 1 program skeleton:
// config and signer-management paths are wired, while receipt verification
// still returns an explicit not-implemented error until canonical CBOR rebuild,
// ed25519 introspection, and PDA hardening are added.

use anchor_lang::prelude::*;
use solana_program::{hash::hash, sysvar::instructions::ID as INSTRUCTIONS_SYSVAR_ID};

declare_id!("52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2");

pub const CONFIG_SEED: &[u8] = b"config";
pub const PROVIDER_SIGNER_SEED: &[u8] = b"provider_signer";
pub const RECEIPT_SEED: &[u8] = b"receipt";
pub const CHALLENGE_SEED: &[u8] = b"challenge";

pub const MAX_REQUEST_NONCE_LEN: usize = 128;
pub const MAX_PROOF_ID_LEN: usize = 128;
pub const MAX_PROVIDER_LEN: usize = 64;
pub const MAX_MODEL_LEN: usize = 255;
pub const MAX_KEY_ID_LEN: usize = 128;
pub const MAX_PROVIDER_REQUEST_ID_LEN: usize = 255;
pub const MAX_PROOF_URL_LEN: usize = 512;

#[program]
pub mod clawfarm_attestation {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        authority: Pubkey,
        pause_authority: Pubkey,
        challenge_resolver: Pubkey,
        challenge_window_seconds: i64,
        response_window_seconds: i64,
    ) -> Result<()> {
        require!(challenge_window_seconds > 0, ErrorCode::InvalidWindow);
        require!(response_window_seconds > 0, ErrorCode::InvalidWindow);

        let config = &mut ctx.accounts.config;
        config.authority = authority;
        config.pause_authority = pause_authority;
        config.challenge_resolver = challenge_resolver;
        config.challenge_window_seconds = challenge_window_seconds;
        config.response_window_seconds = response_window_seconds;
        config.receipt_count = 0;
        config.challenge_count = 0;
        config.is_paused = false;
        config.phase2_enabled = false;
        config.bump = ctx.bumps.config;
        config.reserved = [0; 32];

        emit!(ConfigInitialized {
            authority,
            pause_authority,
            challenge_resolver,
        });
        Ok(())
    }

    pub fn upsert_provider_signer(
        ctx: Context<UpsertProviderSigner>,
        provider_code: String,
        signer: Pubkey,
        key_id: String,
        attester_type_mask: u8,
        valid_from: i64,
        valid_until: i64,
        metadata_hash: [u8; 32],
    ) -> Result<()> {
        validate_provider_code(&provider_code)?;
        validate_key_id(&key_id)?;
        require!(attester_type_mask != 0, ErrorCode::InvalidAttesterTypeMask);
        require!(
            valid_until == 0 || valid_until >= valid_from,
            ErrorCode::InvalidValidityWindow
        );

        let now = Clock::get()?.unix_timestamp;
        let provider_signer = &mut ctx.accounts.provider_signer;
        if provider_signer.created_at == 0 {
            provider_signer.created_at = now;
        }

        provider_signer.provider_code = provider_code.clone();
        provider_signer.signer = signer;
        provider_signer.key_id = key_id.clone();
        provider_signer.attester_type_mask = attester_type_mask;
        provider_signer.status = SignerStatus::Active as u8;
        provider_signer.valid_from = valid_from;
        provider_signer.valid_until = valid_until;
        provider_signer.metadata_hash = metadata_hash;
        provider_signer.updated_at = now;
        provider_signer.bump = 0;
        provider_signer.reserved = [0; 32];

        emit!(ProviderSignerUpserted {
            provider_code,
            signer,
            key_id,
            attester_type_mask,
        });
        Ok(())
    }

    pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.is_paused = is_paused;

        emit!(PauseUpdated { is_paused });
        Ok(())
    }

    pub fn revoke_provider_signer(
        ctx: Context<RevokeProviderSigner>,
        provider_code: String,
        signer: Pubkey,
    ) -> Result<()> {
        validate_provider_code(&provider_code)?;
        let provider_signer = &mut ctx.accounts.provider_signer;
        require!(
            provider_signer.provider_code == provider_code,
            ErrorCode::ProviderMismatch
        );
        require!(provider_signer.signer == signer, ErrorCode::SignerMismatch);

        provider_signer.status = SignerStatus::Revoked as u8;
        provider_signer.updated_at = Clock::get()?.unix_timestamp;

        emit!(ProviderSignerRevoked {
            provider_code,
            signer,
        });
        Ok(())
    }

    pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()> {
        validate_submit_receipt_args(&args)?;

        let config = &ctx.accounts.config;
        require!(!config.is_paused, ErrorCode::ProgramPaused);

        let provider_signer = &ctx.accounts.provider_signer;
        require!(
            provider_signer.status == SignerStatus::Active as u8,
            ErrorCode::SignerInactive
        );
        require!(
            provider_signer.signer == args.signer,
            ErrorCode::SignerMismatch
        );
        require!(
            provider_signer.provider_code == args.provider,
            ErrorCode::ProviderMismatch
        );
        require!(
            provider_signer.attester_type_mask & attester_type_mask(args.attester_type) != 0,
            ErrorCode::SignerAttesterTypeMismatch
        );

        let _instructions_sysvar = &ctx.accounts.instructions_sysvar;
        let _receipt = &ctx.accounts.receipt;
        let _proof_url_hash = hash(args.proof_url.as_bytes()).to_bytes();
        let _request_nonce_seed = request_nonce_seed(args.request_nonce.as_str());

        err!(ErrorCode::ReceiptVerificationNotImplemented)
    }

    pub fn open_challenge(
        ctx: Context<OpenChallenge>,
        request_nonce: String,
        challenge_type: u8,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        validate_request_nonce(&request_nonce)?;
        let challenge_type = ChallengeType::try_from(challenge_type)?;
        let now = Clock::get()?.unix_timestamp;

        let receipt = &mut ctx.accounts.receipt;
        require!(
            receipt.status == ReceiptStatus::Submitted as u8
                || receipt.status == ReceiptStatus::Challenged as u8,
            ErrorCode::ReceiptNotChallengeable
        );
        require!(
            receipt.request_nonce == request_nonce,
            ErrorCode::ReceiptNonceMismatch
        );
        require!(
            now <= receipt.challenge_deadline,
            ErrorCode::ChallengeWindowClosed
        );

        let challenge = &mut ctx.accounts.challenge;
        challenge.request_nonce = request_nonce.clone();
        challenge.receipt = receipt.key();
        challenge.challenger = ctx.accounts.challenger.key();
        challenge.challenge_type = challenge_type as u8;
        challenge.evidence_hash = evidence_hash;
        challenge.response_hash = [0; 32];
        challenge.opened_at = now;
        challenge.response_deadline = now + ctx.accounts.config.response_window_seconds;
        challenge.resolved_at = 0;
        challenge.status = ChallengeStatus::Open as u8;
        challenge.resolution_code = ResolutionCode::None as u8;
        challenge.bump = 0;
        challenge.reserved = [0; 32];

        receipt.status = ReceiptStatus::Challenged as u8;
        ctx.accounts.config.challenge_count = ctx.accounts.config.challenge_count.saturating_add(1);

        emit!(ChallengeOpened {
            request_nonce,
            challenger: ctx.accounts.challenger.key(),
            challenge_type: challenge_type as u8,
        });
        Ok(())
    }

    pub fn respond_challenge(
        ctx: Context<RespondChallenge>,
        request_nonce: String,
        challenge_type: u8,
        challenger: Pubkey,
        response_hash: [u8; 32],
    ) -> Result<()> {
        validate_request_nonce(&request_nonce)?;
        let challenge_type = ChallengeType::try_from(challenge_type)?;
        let now = Clock::get()?.unix_timestamp;

        let challenge = &mut ctx.accounts.challenge;
        require!(
            challenge.request_nonce == request_nonce,
            ErrorCode::ReceiptNonceMismatch
        );
        require!(
            challenge.challenger == challenger,
            ErrorCode::ChallengeChallengerMismatch
        );
        require!(
            challenge.challenge_type == challenge_type as u8,
            ErrorCode::ChallengeTypeMismatch
        );
        require!(
            challenge.status == ChallengeStatus::Open as u8,
            ErrorCode::ChallengeNotOpen
        );
        require!(
            now <= challenge.response_deadline,
            ErrorCode::ResponseWindowClosed
        );

        challenge.response_hash = response_hash;
        challenge.status = ChallengeStatus::Responded as u8;

        emit!(ChallengeResponded {
            request_nonce,
            challenger,
            challenge_type: challenge_type as u8,
        });
        Ok(())
    }

    pub fn resolve_challenge(
        ctx: Context<ResolveChallenge>,
        request_nonce: String,
        challenge_type: u8,
        challenger: Pubkey,
        resolution_code: u8,
    ) -> Result<()> {
        validate_request_nonce(&request_nonce)?;
        let challenge_type = ChallengeType::try_from(challenge_type)?;
        let resolution_code = ResolutionCode::try_from(resolution_code)?;
        require!(
            resolution_code != ResolutionCode::None,
            ErrorCode::ChallengeResolutionInvalid
        );

        let challenge = &mut ctx.accounts.challenge;
        require!(
            challenge.request_nonce == request_nonce,
            ErrorCode::ReceiptNonceMismatch
        );
        require!(
            challenge.challenger == challenger,
            ErrorCode::ChallengeChallengerMismatch
        );
        require!(
            challenge.challenge_type == challenge_type as u8,
            ErrorCode::ChallengeTypeMismatch
        );
        require!(
            challenge.status == ChallengeStatus::Open as u8
                || challenge.status == ChallengeStatus::Responded as u8,
            ErrorCode::ChallengeNotResolvable
        );

        challenge.resolution_code = resolution_code as u8;
        challenge.resolved_at = Clock::get()?.unix_timestamp;

        let receipt = &mut ctx.accounts.receipt;
        match resolution_code {
            ResolutionCode::Accepted | ResolutionCode::ReceiptInvalidated => {
                challenge.status = ChallengeStatus::Accepted as u8;
                receipt.status = ReceiptStatus::Rejected as u8;
            }
            ResolutionCode::SignerRevoked => {
                challenge.status = ChallengeStatus::Accepted as u8;
                receipt.status = ReceiptStatus::Slashed as u8;
            }
            ResolutionCode::Rejected => {
                challenge.status = ChallengeStatus::Rejected as u8;
                receipt.status = ReceiptStatus::Submitted as u8;
            }
            ResolutionCode::None => unreachable!(),
        }

        emit!(ChallengeResolved {
            request_nonce,
            challenger,
            challenge_type: challenge_type as u8,
            resolution_code: resolution_code as u8,
        });
        Ok(())
    }

    pub fn finalize_receipt(ctx: Context<FinalizeReceipt>, request_nonce: String) -> Result<()> {
        validate_request_nonce(&request_nonce)?;
        let now = Clock::get()?.unix_timestamp;
        let receipt = &mut ctx.accounts.receipt;

        require!(
            receipt.request_nonce == request_nonce,
            ErrorCode::ReceiptNonceMismatch
        );
        require!(
            receipt.status == ReceiptStatus::Submitted as u8
                || receipt.status == ReceiptStatus::Challenged as u8,
            ErrorCode::ReceiptNotFinalizable
        );
        require!(
            now > receipt.challenge_deadline,
            ErrorCode::ChallengeWindowOpen
        );

        receipt.status = ReceiptStatus::Finalized as u8;
        receipt.finalized_at = now;

        emit!(ReceiptFinalized {
            request_nonce,
            proof_id: receipt.proof_id.clone(),
            provider: receipt.provider.clone(),
            signer: receipt.signer,
            receipt_hash: receipt.receipt_hash,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpsertProviderSigner<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + ProviderSigner::INIT_SPACE
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetPause<'info> {
    pub pause_authority: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = pause_authority
    )]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
pub struct RevokeProviderSigner<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub provider_signer: Account<'info, ProviderSigner>,
}

#[derive(Accounts)]
pub struct SubmitReceipt<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    pub provider_signer: Account<'info, ProviderSigner>,
    #[account(
        init,
        payer = payer,
        space = 8 + Receipt::INIT_SPACE
    )]
    pub receipt: Account<'info, Receipt>,
    /// CHECK: validated against the instructions sysvar id
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OpenChallenge<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub receipt: Account<'info, Receipt>,
    #[account(
        init,
        payer = challenger,
        space = 8 + Challenge::INIT_SPACE
    )]
    pub challenge: Account<'info, Challenge>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RespondChallenge<'info> {
    pub responder: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    pub receipt: Account<'info, Receipt>,
    #[account(mut)]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
pub struct ResolveChallenge<'info> {
    pub challenge_resolver: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = challenge_resolver
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub receipt: Account<'info, Receipt>,
    #[account(mut)]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
pub struct FinalizeReceipt<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub receipt: Account<'info, Receipt>,
}

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
    #[max_len(MAX_REQUEST_NONCE_LEN)]
    pub request_nonce: String,
    #[max_len(MAX_PROOF_ID_LEN)]
    pub proof_id: String,
    #[max_len(MAX_PROVIDER_LEN)]
    pub provider: String,
    #[max_len(MAX_MODEL_LEN)]
    pub model: String,
    pub proof_mode: u8,
    pub attester_type: u8,
    pub usage_basis: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub charge_atomic: u64,
    pub charge_mint: Pubkey,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
    pub proof_url_hash: [u8; 32],
    pub submitted_at: i64,
    pub challenge_deadline: i64,
    pub finalized_at: i64,
    pub status: u8,
    pub bump: u8,
    pub reserved: [u8; 64],
}

#[account]
#[derive(InitSpace)]
pub struct Challenge {
    #[max_len(MAX_REQUEST_NONCE_LEN)]
    pub request_nonce: String,
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
    pub reserved: [u8; 32],
}

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
    pub proof_url: String,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
    pub signature: [u8; 64],
}

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
    pub key_id: String,
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
pub struct ReceiptFinalized {
    pub request_nonce: String,
    pub proof_id: String,
    pub provider: String,
    pub signer: Pubkey,
    pub receipt_hash: [u8; 32],
}

#[event]
pub struct ChallengeOpened {
    pub request_nonce: String,
    pub challenger: Pubkey,
    pub challenge_type: u8,
}

#[event]
pub struct ChallengeResponded {
    pub request_nonce: String,
    pub challenger: Pubkey,
    pub challenge_type: u8,
}

#[event]
pub struct ChallengeResolved {
    pub request_nonce: String,
    pub challenger: Pubkey,
    pub challenge_type: u8,
    pub resolution_code: u8,
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
    Responded = 1,
    Accepted = 2,
    Rejected = 3,
    Expired = 4,
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

fn validate_submit_receipt_args(args: &SubmitReceiptArgs) -> Result<()> {
    require!(args.version == 1, ErrorCode::InvalidVersion);
    require!(
        args.proof_mode == ProofMode::SigLog as u8,
        ErrorCode::InvalidProofMode
    );
    require!(
        args.usage_basis == UsageBasis::ProviderReported as u8,
        ErrorCode::InvalidUsageBasis
    );
    require!(
        args.total_tokens == args.prompt_tokens.saturating_add(args.completion_tokens),
        ErrorCode::InvalidTokenTotals
    );

    validate_proof_id(&args.proof_id)?;
    validate_request_nonce(&args.request_nonce)?;
    validate_provider_code(&args.provider)?;
    validate_model(&args.model)?;
    validate_proof_url(&args.proof_url)?;

    if let Some(provider_request_id) = &args.provider_request_id {
        require!(
            provider_request_id.len() <= MAX_PROVIDER_REQUEST_ID_LEN,
            ErrorCode::StringTooLong
        );
    }
    if let (Some(issued_at), Some(expires_at)) = (args.issued_at, args.expires_at) {
        require!(expires_at >= issued_at, ErrorCode::ReceiptExpired);
    }
    Ok(())
}

fn validate_request_nonce(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_REQUEST_NONCE_LEN,
        ErrorCode::InvalidRequestNonce
    );
    require!(
        value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        ErrorCode::InvalidRequestNonce
    );
    Ok(())
}

fn validate_proof_id(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_PROOF_ID_LEN,
        ErrorCode::InvalidProofId
    );
    require!(
        value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-')),
        ErrorCode::InvalidProofId
    );
    Ok(())
}

fn validate_provider_code(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_PROVIDER_LEN,
        ErrorCode::InvalidProvider
    );
    Ok(())
}

fn validate_model(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_MODEL_LEN,
        ErrorCode::InvalidModel
    );
    Ok(())
}

fn validate_key_id(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_KEY_ID_LEN,
        ErrorCode::StringTooLong
    );
    Ok(())
}

fn validate_proof_url(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_PROOF_URL_LEN,
        ErrorCode::InvalidProofUrl
    );
    require!(
        value.starts_with("https://") || value.starts_with("http://"),
        ErrorCode::InvalidProofUrl
    );
    Ok(())
}

fn request_nonce_seed(request_nonce: &str) -> [u8; 32] {
    hash(request_nonce.as_bytes()).to_bytes()
}

fn attester_type_mask(attester_type: u8) -> u8 {
    match attester_type {
        0 => 1 << 0,
        1 => 1 << 1,
        2 => 1 << 2,
        _ => 0,
    }
}

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
    #[msg("Invalid token totals")]
    InvalidTokenTotals,
    #[msg("Receipt is expired or has inconsistent timestamps")]
    ReceiptExpired,
    #[msg("Signer is inactive")]
    SignerInactive,
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
    #[msg("Receipt verification is not implemented yet")]
    ReceiptVerificationNotImplemented,
}
