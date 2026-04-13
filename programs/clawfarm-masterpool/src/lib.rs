#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

mod constants;
mod error;
mod instructions;
mod state;
mod utils;

#[allow(unused_imports)]
use instructions::challenge::{
    __client_accounts_record_challenge_bond, __client_accounts_resolve_challenge_economics,
    __cpi_client_accounts_record_challenge_bond, __cpi_client_accounts_resolve_challenge_economics,
};
#[allow(unused_imports)]
use instructions::config::{
    __client_accounts_initialize_masterpool, __client_accounts_mint_genesis_supply,
    __client_accounts_set_pause_flags, __client_accounts_update_config,
    __cpi_client_accounts_initialize_masterpool, __cpi_client_accounts_mint_genesis_supply,
    __cpi_client_accounts_set_pause_flags, __cpi_client_accounts_update_config,
};
#[allow(unused_imports)]
use instructions::provider::{
    __client_accounts_exit_provider, __client_accounts_register_provider,
    __cpi_client_accounts_exit_provider, __cpi_client_accounts_register_provider,
};
#[allow(unused_imports)]
use instructions::receipt::{
    __client_accounts_record_mining_from_receipt, __client_accounts_settle_finalized_receipt,
    __cpi_client_accounts_record_mining_from_receipt, __cpi_client_accounts_settle_finalized_receipt,
};
#[allow(unused_imports)]
use instructions::reward::{
    __client_accounts_claim_released_claw, __client_accounts_materialize_reward_release,
    __cpi_client_accounts_claim_released_claw, __cpi_client_accounts_materialize_reward_release,
};

pub use constants::*;
pub use error::ErrorCode;
pub use instructions::{
    ClaimReleasedClaw, ExitProvider, InitializeMasterpool, MaterializeRewardRelease,
    MintGenesisSupply, RecordChallengeBond, RecordMiningFromReceipt, RecordMiningFromReceiptArgs,
    RegisterProvider, ResolveChallengeEconomics, SetPauseFlags, SettleFinalizedReceipt,
    UpdateConfig,
};
pub use state::*;

declare_id!("3sk574EAo5fhTCaj9hyDou4pgLBV7TgTSWZPyNeA8TLM");

#[program]
pub mod clawfarm_masterpool {
    use super::*;

    pub fn initialize_masterpool(
        ctx: Context<InitializeMasterpool>,
        params: Phase1ConfigParams,
    ) -> Result<()> {
        instructions::config::initialize_masterpool(ctx, params)
    }

    pub fn mint_genesis_supply(ctx: Context<MintGenesisSupply>) -> Result<()> {
        instructions::config::mint_genesis_supply(ctx)
    }

    pub fn update_config(ctx: Context<UpdateConfig>, params: Phase1ConfigParams) -> Result<()> {
        instructions::config::update_config(ctx, params)
    }

    pub fn set_pause_flags(
        ctx: Context<SetPauseFlags>,
        pause_receipt_recording: bool,
        pause_challenge_processing: bool,
        pause_finalization: bool,
        pause_claims: bool,
    ) -> Result<()> {
        instructions::config::set_pause_flags(
            ctx,
            pause_receipt_recording,
            pause_challenge_processing,
            pause_finalization,
            pause_claims,
        )
    }

    pub fn register_provider(ctx: Context<RegisterProvider>) -> Result<()> {
        instructions::provider::register_provider(ctx)
    }

    pub fn exit_provider(ctx: Context<ExitProvider>) -> Result<()> {
        instructions::provider::exit_provider(ctx)
    }

    pub fn materialize_reward_release(
        ctx: Context<MaterializeRewardRelease>,
        amount: u64,
    ) -> Result<()> {
        instructions::reward::materialize_reward_release(ctx, amount)
    }

    pub fn claim_released_claw(ctx: Context<ClaimReleasedClaw>) -> Result<()> {
        instructions::reward::claim_released_claw(ctx)
    }

    pub fn record_mining_from_receipt(
        ctx: Context<RecordMiningFromReceipt>,
        args: RecordMiningFromReceiptArgs,
    ) -> Result<()> {
        instructions::receipt::record_mining_from_receipt(ctx, args)
    }

    pub fn settle_finalized_receipt(
        ctx: Context<SettleFinalizedReceipt>,
        attestation_receipt_status: u8,
    ) -> Result<()> {
        instructions::receipt::settle_finalized_receipt(ctx, attestation_receipt_status)
    }

    pub fn record_challenge_bond(ctx: Context<RecordChallengeBond>) -> Result<()> {
        instructions::challenge::record_challenge_bond(ctx)
    }

    pub fn resolve_challenge_economics(
        ctx: Context<ResolveChallengeEconomics>,
        resolution_code: u8,
    ) -> Result<()> {
        instructions::challenge::resolve_challenge_economics(ctx, resolution_code)
    }
}
