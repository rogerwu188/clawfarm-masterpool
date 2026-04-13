#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

mod constants;
mod error;
mod events;
mod instructions;
mod state;
mod utils;

use instructions::admin::{
    __client_accounts_initialize_config, __client_accounts_revoke_provider_signer,
    __client_accounts_set_pause, __client_accounts_upsert_provider_signer,
};
use instructions::challenge::{
    __client_accounts_close_challenge, __client_accounts_open_challenge,
    __client_accounts_resolve_challenge,
};
use instructions::receipt::{
    __client_accounts_close_receipt, __client_accounts_finalize_receipt,
    __client_accounts_submit_receipt,
};

pub use constants::*;
pub use error::ErrorCode;
pub use events::*;
pub use instructions::{
    CloseChallenge, CloseReceipt, FinalizeReceipt, InitializeConfig, OpenChallenge,
    ResolveChallenge, RevokeProviderSigner, SetPause, SubmitReceipt, UpsertProviderSigner,
};
pub use state::*;

declare_id!("52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2");

#[program]
pub mod clawfarm_attestation {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        authority: Pubkey,
        pause_authority: Pubkey,
        challenge_resolver: Pubkey,
        masterpool_program: Pubkey,
        challenge_window_seconds: i64,
    ) -> Result<()> {
        instructions::admin::initialize_config(
            ctx,
            authority,
            pause_authority,
            challenge_resolver,
            masterpool_program,
            challenge_window_seconds,
        )
    }

    pub fn upsert_provider_signer(
        ctx: Context<UpsertProviderSigner>,
        provider_code: String,
        signer: Pubkey,
        attester_type_mask: u8,
        valid_from: i64,
        valid_until: i64,
    ) -> Result<()> {
        instructions::admin::upsert_provider_signer(
            ctx,
            provider_code,
            signer,
            attester_type_mask,
            valid_from,
            valid_until,
        )
    }

    pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()> {
        instructions::admin::set_pause(ctx, is_paused)
    }

    pub fn revoke_provider_signer(
        ctx: Context<RevokeProviderSigner>,
        provider_code: String,
        signer: Pubkey,
    ) -> Result<()> {
        instructions::admin::revoke_provider_signer(ctx, provider_code, signer)
    }

    pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()> {
        instructions::receipt::submit_receipt(ctx, args)
    }

    pub fn open_challenge(
        ctx: Context<OpenChallenge>,
        challenge_type: u8,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        instructions::challenge::open_challenge(ctx, challenge_type, evidence_hash)
    }

    pub fn resolve_challenge(ctx: Context<ResolveChallenge>, resolution_code: u8) -> Result<()> {
        instructions::challenge::resolve_challenge(ctx, resolution_code)
    }

    pub fn finalize_receipt(ctx: Context<FinalizeReceipt>) -> Result<()> {
        instructions::receipt::finalize_receipt(ctx)
    }

    pub fn close_challenge(ctx: Context<CloseChallenge>) -> Result<()> {
        instructions::challenge::close_challenge(ctx)
    }

    pub fn close_receipt(ctx: Context<CloseReceipt>) -> Result<()> {
        instructions::receipt::close_receipt(ctx)
    }
}

#[cfg(test)]
mod tests;
