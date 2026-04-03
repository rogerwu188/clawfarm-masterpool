use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, PROVIDER_SIGNER_SEED},
    error::ErrorCode,
    events::{ConfigInitialized, PauseUpdated, ProviderSignerRevoked, ProviderSignerUpserted},
    state::{Config, ProviderSigner, SignerStatus},
    utils::{provider_signer_seed, validate_provider_code},
};

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    treasury: Pubkey,
    challenge_window_seconds: i64,
    challenge_bond_lamports: u64,
) -> Result<()> {
    require!(challenge_window_seconds > 0, ErrorCode::InvalidWindow);
    require!(challenge_bond_lamports > 0, ErrorCode::InvalidChallengeBond);
    require!(treasury != Pubkey::default(), ErrorCode::InvalidTreasury);

    let config = &mut ctx.accounts.config;
    config.authority = authority;
    config.pause_authority = pause_authority;
    config.challenge_resolver = challenge_resolver;
    config.treasury = treasury;
    config.challenge_window_seconds = challenge_window_seconds;
    config.challenge_bond_lamports = challenge_bond_lamports;
    config.is_paused = false;

    emit!(ConfigInitialized {
        authority,
        pause_authority,
        challenge_resolver,
        treasury,
        challenge_bond_lamports,
    });
    Ok(())
}

pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_code: String,
    signer: Pubkey,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
) -> Result<()> {
    validate_provider_code(&provider_code)?;
    require!(attester_type_mask != 0, ErrorCode::InvalidAttesterTypeMask);
    require!(
        valid_until == 0 || valid_until >= valid_from,
        ErrorCode::InvalidValidityWindow
    );

    let provider_signer = &mut ctx.accounts.provider_signer;
    provider_signer.attester_type_mask = attester_type_mask;
    provider_signer.status = SignerStatus::Active as u8;
    provider_signer.valid_from = valid_from;
    provider_signer.valid_until = valid_until;

    emit!(ProviderSignerUpserted {
        provider_code,
        signer,
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
    provider_signer.status = SignerStatus::Revoked as u8;

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
        bump,
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
        bump,
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
        bump
    )]
    pub provider_signer: Account<'info, ProviderSigner>,
}
