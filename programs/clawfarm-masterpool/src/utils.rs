use anchor_lang::prelude::*;
use anchor_spl::token::{accessor, Mint, TokenAccount};

use crate::{
    constants::{BPS_SCALE, CLAW_DECIMALS, RATE_SCALE, USDC_DECIMALS},
    error::ErrorCode,
    state::{GlobalConfig, Phase1ConfigParams, RewardAccount, RewardAccountKind},
};

pub fn validate_phase1_params(params: &Phase1ConfigParams) -> Result<()> {
    require!(
        params.exchange_rate_claw_per_usdc_e6 > 0
            && params.provider_stake_usdc > 0
            && params.lock_days > 0
            && params.provider_slash_claw_amount > 0
            && params.challenge_bond_claw_amount > 0,
        ErrorCode::InvalidPositiveAmount
    );
    require!(
        params.provider_usdc_share_bps + params.treasury_usdc_share_bps == BPS_SCALE,
        ErrorCode::InvalidSplitInvariant
    );
    require!(
        params.user_claw_share_bps + params.provider_claw_share_bps == BPS_SCALE,
        ErrorCode::InvalidSplitInvariant
    );
    require!(
        params.challenger_reward_bps + params.burn_bps == BPS_SCALE,
        ErrorCode::InvalidSplitInvariant
    );
    Ok(())
}

pub fn require_phase1_mint_decimals(claw_mint: &Mint, usdc_mint: &Mint) -> Result<()> {
    require!(
        claw_mint.decimals == CLAW_DECIMALS && usdc_mint.decimals == USDC_DECIMALS,
        ErrorCode::InvalidMintDecimals
    );
    Ok(())
}

pub fn calculate_bps_amount(amount: u64, bps: u16) -> Result<u64> {
    ((u128::from(amount) * u128::from(bps)) / u128::from(BPS_SCALE))
        .try_into()
        .map_err(|_| error!(ErrorCode::MathOverflow))
}

pub fn calculate_claw_amount(usdc_amount: u64, rate_e6: u64) -> Result<u64> {
    ((u128::from(usdc_amount) * u128::from(rate_e6)) / u128::from(RATE_SCALE))
        .try_into()
        .map_err(|_| error!(ErrorCode::MathOverflow))
}

pub fn checked_add_u64(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn checked_sub_u64(left: u64, right: u64) -> Result<u64> {
    left.checked_sub(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn checked_add_i64(left: i64, right: i64) -> Result<i64> {
    left.checked_add(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn checked_sub_i64(left: i64, right: i64) -> Result<i64> {
    left.checked_sub(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn require_attestation_caller(
    config: &GlobalConfig,
    attestation_config: &AccountInfo<'_>,
) -> Result<()> {
    require!(
        attestation_config.owner == &config.attestation_program && attestation_config.is_signer,
        ErrorCode::UnauthorizedAttestationCaller
    );
    Ok(())
}

pub fn require_token_owner(token_account: &TokenAccount, owner: &Pubkey) -> Result<()> {
    require!(
        token_account.owner == *owner,
        ErrorCode::InvalidTokenOwner
    );
    Ok(())
}

pub fn require_token_mint(token_account: &TokenAccount, mint: &Pubkey) -> Result<()> {
    require!(token_account.mint == *mint, ErrorCode::InvalidTokenMint);
    Ok(())
}

pub fn require_token_owner_info(token_account: &AccountInfo<'_>, owner: &Pubkey) -> Result<()> {
    require!(
        accessor::authority(token_account)? == *owner,
        ErrorCode::InvalidTokenOwner
    );
    Ok(())
}

pub fn require_token_mint_info(token_account: &AccountInfo<'_>, mint: &Pubkey) -> Result<()> {
    require!(
        accessor::mint(token_account)? == *mint,
        ErrorCode::InvalidTokenMint
    );
    Ok(())
}

pub fn initialize_reward_account(
    reward_account: &mut RewardAccount,
    owner: Pubkey,
    reward_kind: RewardAccountKind,
    now: i64,
) -> Result<()> {
    if !reward_account.initialized {
        reward_account.initialized = true;
        reward_account.owner = owner;
        reward_account.reward_kind = reward_kind.into();
        reward_account.locked_claw_total = 0;
        reward_account.released_claw_total = 0;
        reward_account.claimed_claw_total = 0;
        reward_account.created_at = now;
        reward_account.updated_at = now;
        return Ok(());
    }

    require!(
        reward_account.owner == owner,
        ErrorCode::InvalidRewardAccountOwner
    );
    require!(
        reward_account.reward_kind == u8::from(reward_kind),
        ErrorCode::InvalidRewardAccountKind
    );
    Ok(())
}
