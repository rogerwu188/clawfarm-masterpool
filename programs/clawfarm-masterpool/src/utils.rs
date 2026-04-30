use anchor_lang::prelude::*;
use anchor_spl::token::{accessor, Mint, TokenAccount};

use crate::{
    constants::{BPS_SCALE, CLAW_DECIMALS, MAX_RECEIPT_USDC_ATOMIC, RATE_SCALE, USDC_DECIMALS},
    error::ErrorCode,
    state::{FaucetLimits, GlobalConfig, Phase1ConfigParams, RewardAccount, RewardAccountKind},
};

pub fn validate_phase1_params(params: &Phase1ConfigParams) -> Result<()> {
    let usdc_split_total = u32::from(params.provider_usdc_share_bps)
        .checked_add(u32::from(params.treasury_usdc_share_bps))
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
    let claw_split_total = u32::from(params.user_claw_share_bps)
        .checked_add(u32::from(params.provider_claw_share_bps))
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
    let challenge_split_total = u32::from(params.challenger_reward_bps)
        .checked_add(u32::from(params.burn_bps))
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;

    require!(
        params.exchange_rate_claw_per_usdc_e6 > 0
            && params.provider_stake_usdc > 0
            && params.lock_days > 0
            && params.provider_slash_claw_amount > 0
            && params.challenge_bond_claw_amount > 0,
        ErrorCode::InvalidPositiveAmount
    );
    require!(
        usdc_split_total == u32::from(BPS_SCALE),
        ErrorCode::InvalidSplitInvariant
    );
    require!(
        claw_split_total == u32::from(BPS_SCALE),
        ErrorCode::InvalidSplitInvariant
    );
    require!(
        challenge_split_total == u32::from(BPS_SCALE),
        ErrorCode::InvalidSplitInvariant
    );
    validate_phase1_param_bounds(params)?;
    Ok(())
}

pub fn validate_phase1_param_bounds(params: &Phase1ConfigParams) -> Result<()> {
    let max_treasury_usdc = calculate_bps_amount(
        MAX_RECEIPT_USDC_ATOMIC,
        params.treasury_usdc_share_bps,
    )
    .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let max_provider_usdc = checked_sub_u64(MAX_RECEIPT_USDC_ATOMIC, max_treasury_usdc)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let max_total_claw = calculate_claw_amount(
        MAX_RECEIPT_USDC_ATOMIC,
        params.exchange_rate_claw_per_usdc_e6,
    )
    .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _max_user_claw = calculate_bps_amount(max_total_claw, params.user_claw_share_bps)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let max_provider_claw = checked_sub_u64(max_total_claw, _max_user_claw)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _max_provider_reward_locked = max_provider_claw;
    let _max_provider_pending_usdc = max_provider_usdc;
    let _challenger_reward = calculate_bps_amount(
        params.provider_slash_claw_amount,
        params.challenger_reward_bps,
    )
    .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    let _burn_amount = checked_sub_u64(params.provider_slash_claw_amount, _challenger_reward)
        .map_err(|_| error!(ErrorCode::InvalidGovernanceParameters))?;
    Ok(())
}

pub fn validate_supported_receipt_charge(total_usdc_paid: u64) -> Result<()> {
    require!(
        total_usdc_paid <= MAX_RECEIPT_USDC_ATOMIC,
        ErrorCode::ReceiptChargeTooLarge
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

pub fn checked_add_i128(left: i128, right: i128) -> Result<i128> {
    left.checked_add(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn checked_sub_i128(left: i128, right: i128) -> Result<i128> {
    left.checked_sub(right)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

pub fn validate_faucet_limits(limits: &FaucetLimits) -> Result<()> {
    require!(
        limits.max_claw_per_claim > 0
            && limits.max_usdc_per_claim > 0
            && limits.max_claw_per_wallet_per_day > 0
            && limits.max_usdc_per_wallet_per_day > 0
            && limits.max_claw_global_per_day > 0
            && limits.max_usdc_global_per_day > 0,
        ErrorCode::InvalidFaucetLimits
    );
    require!(
        limits.max_claw_per_claim <= limits.max_claw_per_wallet_per_day
            && limits.max_usdc_per_claim <= limits.max_usdc_per_wallet_per_day,
        ErrorCode::InvalidFaucetLimits
    );
    require!(
        limits.max_claw_per_wallet_per_day <= limits.max_claw_global_per_day
            && limits.max_usdc_per_wallet_per_day <= limits.max_usdc_global_per_day,
        ErrorCode::InvalidFaucetLimits
    );
    Ok(())
}

pub fn compute_linear_releasable_amount(
    total_locked: u64,
    released_so_far: u64,
    lock_start: i64,
    now: i64,
    lock_days: u16,
) -> Result<u64> {
    let lock_seconds = i64::from(lock_days)
        .checked_mul(86_400)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
    require!(lock_seconds > 0, ErrorCode::InvalidPositiveAmount);

    let elapsed = now
        .saturating_sub(lock_start)
        .clamp(0, lock_seconds);
    let vested: u64 = ((u128::from(total_locked)
        * u128::try_from(elapsed).map_err(|_| error!(ErrorCode::MathOverflow))?)
        / u128::try_from(lock_seconds).map_err(|_| error!(ErrorCode::MathOverflow))?)
    .try_into()
    .map_err(|_| error!(ErrorCode::MathOverflow))?;

    checked_sub_u64(vested, released_so_far)
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
        reward_account.pending_claw_total = 0;
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

#[cfg(test)]
mod tests {
    use anchor_lang::error::Error;

    use super::{
        compute_linear_releasable_amount, validate_phase1_param_bounds,
        validate_supported_receipt_charge,
    };
    use crate::{
        constants::{MAX_RECEIPT_USDC_ATOMIC, RATE_SCALE},
        error::ErrorCode,
        state::Phase1ConfigParams,
    };

    #[test]
    fn linear_vesting_releases_only_elapsed_fraction() {
        let releasable =
            compute_linear_releasable_amount(100, 0, 0, 5 * 86_400, 10).unwrap();

        assert_eq!(releasable, 50);
    }

    #[test]
    fn rejects_overflow_prone_phase1_params() {
        let err = validate_phase1_param_bounds(&Phase1ConfigParams {
            exchange_rate_claw_per_usdc_e6: u64::MAX,
            provider_stake_usdc: 100 * RATE_SCALE,
            provider_usdc_share_bps: 700,
            treasury_usdc_share_bps: 300,
            user_claw_share_bps: 300,
            provider_claw_share_bps: 700,
            lock_days: 180,
            provider_slash_claw_amount: RATE_SCALE,
            challenger_reward_bps: 700,
            burn_bps: 300,
            challenge_bond_claw_amount: 10 * RATE_SCALE,
        })
        .unwrap_err();

        assert_anchor_error(err, ErrorCode::InvalidGovernanceParameters);
    }

    #[test]
    fn rejects_receipt_charge_above_supported_domain() {
        let err = validate_supported_receipt_charge(MAX_RECEIPT_USDC_ATOMIC + 1).unwrap_err();

        assert_anchor_error(err, ErrorCode::ReceiptChargeTooLarge);
    }

    fn assert_anchor_error(error: Error, expected: ErrorCode) {
        match error {
            Error::AnchorError(anchor_error) => {
                assert_eq!(anchor_error.error_code_number, expected as u32 + 6000);
            }
            other => panic!("unexpected error type: {other:?}"),
        }
    }
}

#[cfg(test)]
mod faucet_tests {
    use anchor_lang::error::Error;

    use super::validate_faucet_limits;
    use crate::{error::ErrorCode, state::FaucetLimits};

    fn valid_limits() -> FaucetLimits {
        FaucetLimits {
            max_claw_per_claim: 10_000_000,
            max_usdc_per_claim: 10_000_000,
            max_claw_per_wallet_per_day: 50_000_000,
            max_usdc_per_wallet_per_day: 50_000_000,
            max_claw_global_per_day: 50_000_000_000,
            max_usdc_global_per_day: 50_000_000_000,
        }
    }

    #[test]
    fn accepts_valid_faucet_limits() {
        validate_faucet_limits(&valid_limits()).unwrap();
    }

    #[test]
    fn rejects_zero_faucet_limits() {
        let mut limits = valid_limits();
        limits.max_claw_per_claim = 0;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }

    #[test]
    fn rejects_per_claim_above_wallet_daily() {
        let mut limits = valid_limits();
        limits.max_usdc_per_claim = 60_000_000;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }

    #[test]
    fn rejects_wallet_daily_above_global_daily() {
        let mut limits = valid_limits();
        limits.max_claw_per_wallet_per_day = 60_000_000_000;
        let err = validate_faucet_limits(&limits).unwrap_err();
        assert_eq!(err, Error::from(ErrorCode::InvalidFaucetLimits));
    }
}
