#![allow(unexpected_cfgs)]

// Phase 1 program implementation:
// config, signer management, receipt submission, and challenge lifecycle are
// wired for the Phase 1 sig_log/provider_reported path.

use anchor_lang::prelude::*;

mod constants;
mod error;
mod events;
mod instructions;
mod state;
mod utils;

pub use constants::*;
pub use error::ErrorCode;
pub use events::*;
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
        challenge_window_seconds: i64,
    ) -> Result<()> {
        instructions::admin::initialize_config(
            ctx,
            authority,
            pause_authority,
            challenge_resolver,
            challenge_window_seconds,
        )
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
        instructions::admin::upsert_provider_signer(
            ctx,
            provider_code,
            signer,
            key_id,
            attester_type_mask,
            valid_from,
            valid_until,
            metadata_hash,
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
        request_nonce: String,
        challenge_type: u8,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        instructions::challenge::open_challenge(ctx, request_nonce, challenge_type, evidence_hash)
    }

    pub fn resolve_challenge(
        ctx: Context<ResolveChallenge>,
        request_nonce: String,
        challenge_type: u8,
        challenger: Pubkey,
        resolution_code: u8,
    ) -> Result<()> {
        instructions::challenge::resolve_challenge(
            ctx,
            request_nonce,
            challenge_type,
            challenger,
            resolution_code,
        )
    }

    pub fn finalize_receipt(ctx: Context<FinalizeReceipt>, request_nonce: String) -> Result<()> {
        instructions::receipt::finalize_receipt(ctx, request_nonce)
    }

    pub fn close_challenge(
        ctx: Context<CloseChallenge>,
        request_nonce: String,
        challenge_type: u8,
        challenger: Pubkey,
    ) -> Result<()> {
        instructions::challenge::close_challenge(ctx, request_nonce, challenge_type, challenger)
    }

    pub fn close_receipt(ctx: Context<CloseReceipt>, request_nonce: String) -> Result<()> {
        instructions::receipt::close_receipt(ctx, request_nonce)
    }
}

#[cfg(test)]
mod tests;
