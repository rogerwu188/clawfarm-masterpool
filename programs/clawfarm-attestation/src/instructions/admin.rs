use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, PROVIDER_SIGNER_SEED},
    error::ErrorCode,
    events::{ConfigInitialized, PauseUpdated, ProviderSignerRevoked, ProviderSignerUpserted},
    state::{Config, ProviderSigner, SignerStatus},
    utils::{provider_signer_seed, validate_key_id, validate_provider_code},
};

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    challenge_window_seconds: i64,
) -> Result<()> {
    require!(challenge_window_seconds > 0, ErrorCode::InvalidWindow);

    let config = &mut ctx.accounts.config;
    config.authority = authority;
    config.pause_authority = pause_authority;
    config.challenge_resolver = challenge_resolver;
    config.challenge_window_seconds = challenge_window_seconds;
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
