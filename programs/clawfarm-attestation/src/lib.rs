#![allow(unexpected_cfgs)]

// Phase 1 program implementation:
// config, signer management, receipt submission, and challenge lifecycle are
// wired for the Phase 1 sig_log/provider_reported path.

use anchor_lang::{
    prelude::*,
    solana_program::sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked,
    },
};
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
        provider_signer.bump = ctx.bumps.provider_signer;
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

        let now = Clock::get()?.unix_timestamp;
        let config = &mut ctx.accounts.config;
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
        require!(
            now >= provider_signer.valid_from,
            ErrorCode::SignerNotYetValid
        );
        require!(
            provider_signer.valid_until == 0 || now <= provider_signer.valid_until,
            ErrorCode::SignerExpired
        );

        let canonical_cbor = build_phase1_canonical_cbor(&args)?;
        let computed_receipt_hash = hash(&canonical_cbor).to_bytes();
        require!(
            computed_receipt_hash == args.receipt_hash,
            ErrorCode::ReceiptHashMismatch
        );

        verify_preceding_ed25519_instruction(
            &ctx.accounts.instructions_sysvar.to_account_info(),
            &args.signer,
            &args.signature,
            &args.receipt_hash,
        )?;

        let receipt = &mut ctx.accounts.receipt;
        receipt.request_nonce = args.request_nonce.clone();
        receipt.proof_id = args.proof_id.clone();
        receipt.provider = args.provider.clone();
        receipt.model = args.model.clone();
        receipt.proof_mode = args.proof_mode;
        receipt.attester_type = args.attester_type;
        receipt.usage_basis = args.usage_basis;
        receipt.prompt_tokens = args.prompt_tokens;
        receipt.completion_tokens = args.completion_tokens;
        receipt.total_tokens = args.total_tokens;
        receipt.charge_atomic = args.charge_atomic;
        receipt.charge_mint = args.charge_mint;
        receipt.receipt_hash = args.receipt_hash;
        receipt.signer = args.signer;
        receipt.proof_url_hash = hash(args.proof_url.as_bytes()).to_bytes();
        receipt.submitted_at = now;
        receipt.challenge_deadline = now.saturating_add(config.challenge_window_seconds);
        receipt.finalized_at = 0;
        receipt.status = ReceiptStatus::Submitted as u8;
        receipt.bump = ctx.bumps.receipt;
        receipt.reserved = [0; 64];

        config.receipt_count = config.receipt_count.saturating_add(1);

        emit!(ReceiptSubmitted {
            request_nonce: args.request_nonce,
            proof_id: args.proof_id,
            provider: args.provider,
            signer: args.signer,
            receipt_hash: args.receipt_hash,
        });
        Ok(())
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
            receipt.status == ReceiptStatus::Submitted as u8,
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
        challenge.bump = ctx.bumps.challenge;
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
        require!(
            ctx.accounts.responder.key() == ctx.accounts.config.authority
                || ctx.accounts.responder.key() == ctx.accounts.config.challenge_resolver,
            ErrorCode::ChallengeResponderUnauthorized
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
            receipt.status == ReceiptStatus::Submitted as u8,
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
#[instruction(
    provider_code: String,
    signer: Pubkey,
    key_id: String,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
    metadata_hash: [u8; 32]
)]
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
        space = 8 + ProviderSigner::INIT_SPACE,
        seeds = [
            PROVIDER_SIGNER_SEED,
            &provider_signer_seed(provider_code.as_str()),
            signer.as_ref()
        ],
        bump
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
#[instruction(provider_code: String, signer: Pubkey)]
pub struct RevokeProviderSigner<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [
            PROVIDER_SIGNER_SEED,
            &provider_signer_seed(provider_code.as_str()),
            signer.as_ref()
        ],
        bump = provider_signer.bump
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
}

#[derive(Accounts)]
#[instruction(args: SubmitReceiptArgs)]
pub struct SubmitReceipt<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [
            PROVIDER_SIGNER_SEED,
            &provider_signer_seed(args.provider.as_str()),
            args.signer.as_ref()
        ],
        bump = provider_signer.bump
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
    #[account(
        init,
        payer = payer,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [
            RECEIPT_SEED,
            &request_nonce_seed(args.request_nonce.as_str())
        ],
        bump
    )]
    pub receipt: Account<'info, Receipt>,
    /// CHECK: validated against the instructions sysvar id
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(request_nonce: String, challenge_type: u8, evidence_hash: [u8; 32])]
pub struct OpenChallenge<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            &request_nonce_seed(request_nonce.as_str())
        ],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        init,
        payer = challenger,
        space = 8 + Challenge::INIT_SPACE,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.key().as_ref()
        ],
        bump
    )]
    pub challenge: Account<'info, Challenge>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    response_hash: [u8; 32]
)]
pub struct RespondChallenge<'info> {
    pub responder: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [
            RECEIPT_SEED,
            &request_nonce_seed(request_nonce.as_str())
        ],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.as_ref()
        ],
        bump = challenge.bump
    )]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
#[instruction(
    request_nonce: String,
    challenge_type: u8,
    challenger: Pubkey,
    resolution_code: u8
)]
pub struct ResolveChallenge<'info> {
    pub challenge_resolver: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = challenge_resolver
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            &request_nonce_seed(request_nonce.as_str())
        ],
        bump = receipt.bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        seeds = [
            CHALLENGE_SEED,
            receipt.key().as_ref(),
            &[challenge_type],
            challenger.as_ref()
        ],
        bump = challenge.bump
    )]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
#[instruction(request_nonce: String)]
pub struct FinalizeReceipt<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            &request_nonce_seed(request_nonce.as_str())
        ],
        bump = receipt.bump
    )]
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
pub struct ReceiptSubmitted {
    pub request_nonce: String,
    pub proof_id: String,
    pub provider: String,
    pub signer: Pubkey,
    pub receipt_hash: [u8; 32],
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
    require!(
        attester_type_label(args.attester_type).is_some(),
        ErrorCode::InvalidAttesterType
    );

    if let Some(provider_request_id) = &args.provider_request_id {
        require!(
            provider_request_id.len() <= MAX_PROVIDER_REQUEST_ID_LEN,
            ErrorCode::StringTooLong
        );
    }
    if let Some(issued_at) = args.issued_at {
        require!(issued_at >= 0, ErrorCode::ReceiptExpired);
    }
    if let Some(expires_at) = args.expires_at {
        require!(expires_at >= 0, ErrorCode::ReceiptExpired);
    }
    if let (Some(issued_at), Some(expires_at)) = (args.issued_at, args.expires_at) {
        require!(expires_at >= issued_at, ErrorCode::ReceiptExpired);
    }
    if let Some(http_status) = args.http_status {
        require!(
            (100..=599).contains(&http_status),
            ErrorCode::InvalidHttpStatus
        );
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

fn provider_signer_seed(provider_code: &str) -> [u8; 32] {
    hash(provider_code.as_bytes()).to_bytes()
}

fn attester_type_mask(attester_type: u8) -> u8 {
    match attester_type {
        0 => 1 << 0,
        1 => 1 << 1,
        2 => 1 << 2,
        _ => 0,
    }
}

fn proof_mode_label(proof_mode: u8) -> Option<&'static str> {
    match proof_mode {
        x if x == ProofMode::SigLog as u8 => Some("sig_log"),
        x if x == ProofMode::SigLogZkReserved as u8 => Some("sig_log_zk"),
        _ => None,
    }
}

fn attester_type_label(attester_type: u8) -> Option<&'static str> {
    match attester_type {
        x if x == AttesterType::Provider as u8 => Some("provider"),
        x if x == AttesterType::Gateway as u8 => Some("gateway"),
        x if x == AttesterType::Hybrid as u8 => Some("hybrid"),
        _ => None,
    }
}

fn usage_basis_label(usage_basis: u8) -> Option<&'static str> {
    match usage_basis {
        x if x == UsageBasis::ProviderReported as u8 => Some("provider_reported"),
        x if x == UsageBasis::ServerEstimatedReserved as u8 => Some("server_estimated"),
        x if x == UsageBasis::HybridReserved as u8 => Some("hybrid"),
        x if x == UsageBasis::TokenizerVerifiedReserved as u8 => Some("tokenizer_verified"),
        _ => None,
    }
}

fn build_phase1_canonical_cbor(args: &SubmitReceiptArgs) -> Result<Vec<u8>> {
    let proof_mode = proof_mode_label(args.proof_mode).ok_or_else(|| error!(ErrorCode::InvalidProofMode))?;
    let attester_type =
        attester_type_label(args.attester_type).ok_or_else(|| error!(ErrorCode::InvalidAttesterType))?;
    let usage_basis =
        usage_basis_label(args.usage_basis).ok_or_else(|| error!(ErrorCode::InvalidUsageBasis))?;

    let mut entries = vec![
        (
            "version",
            CanonicalValue::Unsigned(u64::from(args.version)),
        ),
        ("proof_mode", CanonicalValue::Text(proof_mode.to_string())),
        ("proof_id", CanonicalValue::Text(args.proof_id.clone())),
        (
            "request_nonce",
            CanonicalValue::Text(args.request_nonce.clone()),
        ),
        ("provider", CanonicalValue::Text(args.provider.clone())),
        (
            "attester_type",
            CanonicalValue::Text(attester_type.to_string()),
        ),
        ("model", CanonicalValue::Text(args.model.clone())),
        (
            "usage_basis",
            CanonicalValue::Text(usage_basis.to_string()),
        ),
        (
            "prompt_tokens",
            CanonicalValue::Unsigned(args.prompt_tokens),
        ),
        (
            "completion_tokens",
            CanonicalValue::Unsigned(args.completion_tokens),
        ),
        ("total_tokens", CanonicalValue::Unsigned(args.total_tokens)),
        (
            "charge_atomic",
            CanonicalValue::Text(args.charge_atomic.to_string()),
        ),
        (
            "charge_mint",
            CanonicalValue::Text(args.charge_mint.to_string()),
        ),
    ];

    if let Some(provider_request_id) = &args.provider_request_id {
        entries.push((
            "provider_request_id",
            CanonicalValue::Text(provider_request_id.clone()),
        ));
    }
    if let Some(issued_at) = args.issued_at {
        entries.push(("issued_at", CanonicalValue::Signed(issued_at)));
    }
    if let Some(expires_at) = args.expires_at {
        entries.push(("expires_at", CanonicalValue::Signed(expires_at)));
    }
    if let Some(http_status) = args.http_status {
        entries.push(("http_status", CanonicalValue::Unsigned(u64::from(http_status))));
    }
    if let Some(latency_ms) = args.latency_ms {
        entries.push(("latency_ms", CanonicalValue::Unsigned(latency_ms)));
    }

    entries.sort_by(|(left_key, _), (right_key, _)| {
        encode_cbor_text(left_key)
            .cmp(&encode_cbor_text(right_key))
    });

    let mut out = Vec::new();
    encode_cbor_major_len(5, entries.len() as u64, &mut out);
    for (key, value) in entries {
        out.extend_from_slice(&encode_cbor_text(key));
        encode_canonical_value(&value, &mut out);
    }
    Ok(out)
}

fn verify_preceding_ed25519_instruction(
    instructions_sysvar: &AccountInfo<'_>,
    signer: &Pubkey,
    signature: &[u8; 64],
    message: &[u8; 32],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| error!(ErrorCode::MissingEd25519Instruction))?;
    require!(current_index > 0, ErrorCode::MissingEd25519Instruction);

    let ix = load_instruction_at_checked(usize::from(current_index - 1), instructions_sysvar)
        .map_err(|_| error!(ErrorCode::MissingEd25519Instruction))?;
    require!(
        ix.program_id == solana_program::ed25519_program::id(),
        ErrorCode::MissingEd25519Instruction
    );
    require!(ix.accounts.is_empty(), ErrorCode::Ed25519InstructionMismatch);

    let data = ix.data.as_slice();
    require!(data.len() >= 16, ErrorCode::Ed25519InstructionMismatch);
    require!(data[0] == 1, ErrorCode::Ed25519InstructionMismatch);

    let signature_offset = read_u16_le(data, 2)? as usize;
    let signature_instruction_index = read_u16_le(data, 4)?;
    let public_key_offset = read_u16_le(data, 6)? as usize;
    let public_key_instruction_index = read_u16_le(data, 8)?;
    let message_data_offset = read_u16_le(data, 10)? as usize;
    let message_data_size = read_u16_le(data, 12)? as usize;
    let message_instruction_index = read_u16_le(data, 14)?;

    require!(
        signature_instruction_index == u16::MAX
            && public_key_instruction_index == u16::MAX
            && message_instruction_index == u16::MAX,
        ErrorCode::Ed25519InstructionMismatch
    );
    require!(message_data_size == 32, ErrorCode::Ed25519InstructionMismatch);

    let public_key_bytes = read_slice(data, public_key_offset, 32)?;
    let signature_bytes = read_slice(data, signature_offset, 64)?;
    let message_bytes = read_slice(data, message_data_offset, message_data_size)?;

    require!(
        public_key_bytes == signer.as_ref(),
        ErrorCode::Ed25519InstructionMismatch
    );
    require!(
        signature_bytes == signature.as_slice(),
        ErrorCode::Ed25519InstructionMismatch
    );
    require!(
        message_bytes == message.as_slice(),
        ErrorCode::Ed25519InstructionMismatch
    );
    Ok(())
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = read_slice(data, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_slice(data: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| error!(ErrorCode::Ed25519InstructionMismatch))?;
    require!(end <= data.len(), ErrorCode::Ed25519InstructionMismatch);
    Ok(&data[offset..end])
}

enum CanonicalValue {
    Unsigned(u64),
    Signed(i64),
    Text(String),
}

fn encode_canonical_value(value: &CanonicalValue, out: &mut Vec<u8>) {
    match value {
        CanonicalValue::Unsigned(value) => encode_cbor_major_len(0, *value, out),
        CanonicalValue::Signed(value) => encode_cbor_signed(*value, out),
        CanonicalValue::Text(value) => {
            out.extend_from_slice(&encode_cbor_text(value));
        }
    }
}

fn encode_cbor_text(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(9 + bytes.len());
    encode_cbor_major_len(3, bytes.len() as u64, &mut out);
    out.extend_from_slice(bytes);
    out
}

fn encode_cbor_signed(value: i64, out: &mut Vec<u8>) {
    if value >= 0 {
        encode_cbor_major_len(0, value as u64, out);
    } else {
        let encoded = (-1_i128 - i128::from(value)) as u64;
        encode_cbor_major_len(1, encoded, out);
    }
}

fn encode_cbor_major_len(major: u8, len: u64, out: &mut Vec<u8>) {
    match len {
        0..=23 => out.push((major << 5) | (len as u8)),
        24..=0xff => out.extend_from_slice(&[(major << 5) | 24, len as u8]),
        0x100..=0xffff => {
            out.push((major << 5) | 25);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push((major << 5) | 26);
            out.extend_from_slice(&(len as u32).to_be_bytes());
        }
        _ => {
            out.push((major << 5) | 27);
            out.extend_from_slice(&len.to_be_bytes());
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::sysvar::instructions::{
        construct_instructions_data, BorrowedInstruction,
    };
    use solana_program::{
        account_info::AccountInfo,
        clock::Epoch,
        instruction::{Instruction, AccountMeta},
    };

    #[test]
    fn canonical_cbor_omits_non_payload_fields_and_absent_optionals() {
        let args = sample_submit_receipt_args();
        let encoded = build_phase1_canonical_cbor(&args).unwrap();

        assert!(contains_subslice(&encoded, b"proof_mode"));
        assert!(!contains_subslice(&encoded, b"proof_url"));
        assert!(!contains_subslice(&encoded, b"signer"));
        assert!(!contains_subslice(&encoded, b"signature"));
        assert!(!contains_subslice(&encoded, b"receipt_hash"));
        assert!(!contains_subslice(&encoded, b"provider_request_id"));
        assert!(!contains_subslice(&encoded, b"http_status"));
    }

    #[test]
    fn canonical_cbor_includes_present_optional_fields() {
        let mut args = sample_submit_receipt_args();
        args.provider_request_id = Some("req_123".to_string());
        args.issued_at = Some(1_711_950_000);
        args.expires_at = Some(1_711_953_600);
        args.http_status = Some(200);
        args.latency_ms = Some(1_840);

        let encoded = build_phase1_canonical_cbor(&args).unwrap();

        assert!(contains_subslice(&encoded, b"provider_request_id"));
        assert!(contains_subslice(&encoded, b"issued_at"));
        assert!(contains_subslice(&encoded, b"expires_at"));
        assert!(contains_subslice(&encoded, b"http_status"));
        assert!(contains_subslice(&encoded, b"latency_ms"));
    }

    #[test]
    fn preceding_ed25519_instruction_must_match_receipt_args() {
        let signer = Pubkey::new_from_array([7; 32]);
        let signature = [9; 64];
        let message = [5; 32];
        let ed25519_ix = Instruction {
            program_id: solana_program::ed25519_program::id(),
            accounts: vec![],
            data: build_test_ed25519_ix_data(&signer, &signature, &message),
        };
        let submit_ix = Instruction {
            program_id: crate::ID,
            accounts: vec![AccountMeta::new(Pubkey::new_unique(), true)],
            data: vec![0],
        };
        let borrowed = [borrow_instruction(&ed25519_ix), borrow_instruction(&submit_ix)];
        let mut sysvar_data = construct_instructions_data(&borrowed);
        let len = sysvar_data.len();
        sysvar_data[len - 2..len].copy_from_slice(&1u16.to_le_bytes());

        let key = INSTRUCTIONS_SYSVAR_ID;
        let owner = Pubkey::default();
        let mut lamports = 0;
        let account_info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            sysvar_data.as_mut_slice(),
            &owner,
            false,
            Epoch::default(),
        );

        verify_preceding_ed25519_instruction(&account_info, &signer, &signature, &message)
            .unwrap();
        assert!(
            verify_preceding_ed25519_instruction(
                &account_info,
                &Pubkey::new_from_array([8; 32]),
                &signature,
                &message
            )
            .is_err()
        );
    }

    fn sample_submit_receipt_args() -> SubmitReceiptArgs {
        SubmitReceiptArgs {
            version: 1,
            proof_mode: ProofMode::SigLog as u8,
            proof_id: "cap_test_001".to_string(),
            request_nonce: "cfn_test_001".to_string(),
            provider: "unipass".to_string(),
            attester_type: AttesterType::Gateway as u8,
            model: "openai/gpt-4.1".to_string(),
            usage_basis: UsageBasis::ProviderReported as u8,
            prompt_tokens: 123,
            completion_tokens: 456,
            total_tokens: 579,
            charge_atomic: 1_250_000,
            charge_mint: Pubkey::new_unique(),
            provider_request_id: None,
            issued_at: None,
            expires_at: None,
            http_status: None,
            latency_ms: None,
            proof_url: "https://provider.example.com/api/public/v1/proofs/cap_test_001"
                .to_string(),
            receipt_hash: [0; 32],
            signer: Pubkey::new_unique(),
            signature: [0; 64],
        }
    }

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|window| window == needle)
    }

    fn borrow_instruction(ix: &Instruction) -> BorrowedInstruction<'_> {
        BorrowedInstruction {
            program_id: &ix.program_id,
            accounts: ix
                .accounts
                .iter()
                .map(|meta| anchor_lang::solana_program::sysvar::instructions::BorrowedAccountMeta {
                    pubkey: &meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data: ix.data.as_slice(),
        }
    }

    fn build_test_ed25519_ix_data(
        signer: &Pubkey,
        signature: &[u8; 64],
        message: &[u8; 32],
    ) -> Vec<u8> {
        let public_key_offset = 16u16;
        let signature_offset = public_key_offset + 32;
        let message_data_offset = signature_offset + 64;

        let mut data = Vec::with_capacity(16 + 32 + 64 + 32);
        data.extend_from_slice(&[1, 0]);
        data.extend_from_slice(&signature_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&public_key_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&message_data_offset.to_le_bytes());
        data.extend_from_slice(&(message.len() as u16).to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(signer.as_ref());
        data.extend_from_slice(signature);
        data.extend_from_slice(message);
        data
    }
}
