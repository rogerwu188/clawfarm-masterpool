use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, PROVIDER_SIGNER_SEED},
    error::ErrorCode,
    events::{ConfigInitialized, PauseUpdated, ProviderSignerRevoked, ProviderSignerUpserted},
    state::{Config, ProviderSigner, SignerStatus},
};

pub(crate) fn validate_initialize_config_authorities(
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
) -> Result<()> {
    require!(authority != Pubkey::default(), ErrorCode::InvalidAuthority);
    require!(
        pause_authority != Pubkey::default(),
        ErrorCode::InvalidPauseAuthority
    );
    require!(
        challenge_resolver != Pubkey::default(),
        ErrorCode::InvalidChallengeResolver
    );
    Ok(())
}

pub(crate) fn validate_provider_signer_keys(signer: Pubkey, provider_wallet: Pubkey) -> Result<()> {
    require!(signer != Pubkey::default(), ErrorCode::InvalidSigner);
    require!(
        provider_wallet != Pubkey::default(),
        ErrorCode::InvalidProviderWallet
    );
    Ok(())
}

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    masterpool_program: Pubkey,
    challenge_window_seconds: i64,
    challenge_resolution_timeout_seconds: i64,
) -> Result<()> {
    require!(challenge_window_seconds > 0, ErrorCode::InvalidWindow);
    require!(
        challenge_resolution_timeout_seconds > 0,
        ErrorCode::InvalidWindow
    );
    require!(
        masterpool_program != Pubkey::default(),
        ErrorCode::InvalidMasterpoolProgram
    );
    validate_initialize_config_authorities(authority, pause_authority, challenge_resolver)?;

    let config = &mut ctx.accounts.config;
    config.authority = authority;
    config.pause_authority = pause_authority;
    config.challenge_resolver = challenge_resolver;
    config.masterpool_program = masterpool_program;
    config.challenge_window_seconds = challenge_window_seconds;
    config.challenge_resolution_timeout_seconds = challenge_resolution_timeout_seconds;
    config.is_paused = false;

    emit!(ConfigInitialized {
        authority,
        pause_authority,
        challenge_resolver,
        masterpool_program,
    });
    Ok(())
}

pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    signer: Pubkey,
    provider_wallet: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()> {
    validate_provider_signer_keys(signer, provider_wallet)?;
    require!(attester_type_mask != 0, ErrorCode::InvalidAttesterTypeMask);
    require!(
        valid_until == 0 || valid_until >= valid_from,
        ErrorCode::InvalidValidityWindow
    );

    let provider_signer = &mut ctx.accounts.provider_signer;
    provider_signer.signer = signer;
    provider_signer.provider_wallet = provider_wallet;
    provider_signer.attester_type_mask = attester_type_mask;
    provider_signer.status = SignerStatus::Active as u8;
    provider_signer.valid_from = valid_from;
    provider_signer.valid_until = valid_until;

    emit!(ProviderSignerUpserted {
        signer,
        provider_wallet,
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
    signer: Pubkey,
    provider_wallet: Pubkey,
) -> Result<()> {
    let provider_signer = &mut ctx.accounts.provider_signer;
    provider_signer.status = SignerStatus::Revoked as u8;

    emit!(ProviderSignerRevoked {
        signer,
        provider_wallet,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        init,
        payer = initializer,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        constraint = self_program.programdata_address()? == Some(self_program_data.key())
            @ ErrorCode::InvalidProgramData,
    )]
    pub self_program: Program<'info, crate::program::ClawfarmAttestation>,
    #[account(
        constraint = self_program_data.upgrade_authority_address == Some(initializer.key())
            @ ErrorCode::UnauthorizedInitializer,
    )]
    pub self_program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    signer: Pubkey,
    provider_wallet: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64
)]
pub struct UpsertProviderSigner<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + ProviderSigner::INIT_SPACE,
        seeds = [
            PROVIDER_SIGNER_SEED,
            provider_wallet.as_ref(),
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
        bump,
        has_one = pause_authority
    )]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
#[instruction(signer: Pubkey, provider_wallet: Pubkey)]
pub struct RevokeProviderSigner<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [
            PROVIDER_SIGNER_SEED,
            provider_wallet.as_ref(),
            signer.as_ref()
        ],
        bump
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
}
