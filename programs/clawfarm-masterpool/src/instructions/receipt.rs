use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, Token, TokenAccount, TransferChecked,
};

use crate::{
    constants::{
        CONFIG_SEED, POOL_AUTHORITY_SEED, PROVIDER_REWARD_SEED, PROVIDER_SEED,
        RECEIPT_SETTLEMENT_SEED, USER_REWARD_SEED, ATTESTATION_RECEIPT_STATUS_FINALIZED,
    },
    error::ErrorCode,
    state::{
        GlobalConfig, ProviderAccount, ReceiptSettlement, ReceiptSettlementStatus, RewardAccount,
        RewardAccountKind, RECEIPT_SETTLEMENT_SPACE, REWARD_ACCOUNT_SPACE,
    },
    utils::{
        calculate_bps_amount, calculate_claw_amount, checked_add_i64, checked_add_u64,
        checked_sub_u64, initialize_reward_account, require_attestation_caller,
        require_token_mint, require_token_owner,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordMiningFromReceiptArgs {
    pub total_usdc_paid: u64,
    pub charge_mint: Pubkey,
}

pub fn record_mining_from_receipt(
    ctx: Context<RecordMiningFromReceipt>,
    args: RecordMiningFromReceiptArgs,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require_attestation_caller(config, &ctx.accounts.attestation_config.to_account_info())?;
    require!(!config.pause_receipt_recording, ErrorCode::ReceiptRecordingPaused);
    require!(args.total_usdc_paid > 0, ErrorCode::InvalidPositiveAmount);
    require!(args.charge_mint == config.usdc_mint, ErrorCode::ChargeMintMismatch);

    let provider_wallet = ctx.accounts.provider_wallet.key();
    let payer_user = ctx.accounts.payer_user.key();
    require_token_owner(&ctx.accounts.payer_usdc_token, &payer_user)?;
    require_token_mint(&ctx.accounts.payer_usdc_token, &config.usdc_mint)?;

    let provider = &mut ctx.accounts.provider_account;
    require!(
        provider.provider_wallet == provider_wallet,
        ErrorCode::InvalidProviderAccount
    );

    let treasury_share = calculate_bps_amount(args.total_usdc_paid, config.treasury_usdc_share_bps)?;
    let provider_share = checked_sub_u64(args.total_usdc_paid, treasury_share)?;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.payer_usdc_token.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.treasury_usdc_vault.to_account_info(),
                authority: ctx.accounts.payer_user.to_account_info(),
            },
        ),
        treasury_share,
        ctx.accounts.usdc_mint.decimals,
    )?;
    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.payer_usdc_token.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.provider_pending_usdc_vault.to_account_info(),
                authority: ctx.accounts.payer_user.to_account_info(),
            },
        ),
        provider_share,
        ctx.accounts.usdc_mint.decimals,
    )?;

    let total_claw_reward = calculate_claw_amount(
        args.total_usdc_paid,
        config.exchange_rate_claw_per_usdc_e6,
    )?;
    let user_claw = calculate_bps_amount(total_claw_reward, config.user_claw_share_bps)?;
    let provider_claw_total = checked_sub_u64(total_claw_reward, user_claw)?;

    let now = Clock::get()?.unix_timestamp;
    initialize_reward_account(
        &mut ctx.accounts.user_reward_account,
        payer_user,
        RewardAccountKind::User,
        now,
    )?;
    initialize_reward_account(
        &mut ctx.accounts.provider_reward_account,
        provider_wallet,
        RewardAccountKind::Provider,
        now,
    )?;

    ctx.accounts.user_reward_account.locked_claw_total = checked_add_u64(
        ctx.accounts.user_reward_account.locked_claw_total,
        user_claw,
    )?;
    ctx.accounts.user_reward_account.updated_at = now;

    let mut provider_debt_offset = 0_u64;
    let mut provider_locked_claw = provider_claw_total;
    if provider.claw_net_position < 0 {
        let provider_debt = provider.claw_net_position.unsigned_abs();
        provider_debt_offset = provider_debt.min(provider_claw_total);
        provider_locked_claw = checked_sub_u64(provider_claw_total, provider_debt_offset)?;
        provider.claw_net_position = checked_add_i64(
            provider.claw_net_position,
            provider_debt_offset as i64,
        )?;
    }
    if provider_locked_claw > 0 {
        ctx.accounts.provider_reward_account.locked_claw_total = checked_add_u64(
            ctx.accounts.provider_reward_account.locked_claw_total,
            provider_locked_claw,
        )?;
        provider.claw_net_position = checked_add_i64(
            provider.claw_net_position,
            provider_locked_claw as i64,
        )?;
        ctx.accounts.provider_reward_account.updated_at = now;
    }

    provider.pending_provider_usdc = checked_add_u64(provider.pending_provider_usdc, provider_share)?;
    provider.unsettled_receipt_count = checked_add_u64(provider.unsettled_receipt_count, 1)?;
    provider.updated_at = now;

    let settlement = &mut ctx.accounts.receipt_settlement;
    settlement.attestation_receipt = ctx.accounts.attestation_receipt.key();
    settlement.payer_user = payer_user;
    settlement.provider_wallet = provider_wallet;
    settlement.usdc_total_paid = args.total_usdc_paid;
    settlement.usdc_to_provider = provider_share;
    settlement.usdc_to_treasury = treasury_share;
    settlement.claw_to_user = user_claw;
    settlement.claw_to_provider_total = provider_claw_total;
    settlement.claw_provider_debt_offset = provider_debt_offset;
    settlement.claw_to_provider_locked = provider_locked_claw;
    settlement.status = ReceiptSettlementStatus::Recorded.into();
    settlement.created_at = now;
    settlement.updated_at = now;

    Ok(())
}

pub fn settle_finalized_receipt(
    ctx: Context<SettleFinalizedReceipt>,
    attestation_receipt_status: u8,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require_attestation_caller(config, &ctx.accounts.attestation_config.to_account_info())?;
    require!(!config.pause_finalization, ErrorCode::FinalizationPaused);
    require!(
        attestation_receipt_status == ATTESTATION_RECEIPT_STATUS_FINALIZED,
        ErrorCode::InvalidAttestationReceiptStatus
    );

    let settlement = &mut ctx.accounts.receipt_settlement;
    require!(
        settlement.attestation_receipt == ctx.accounts.attestation_receipt.key(),
        ErrorCode::InvalidReceiptSettlement
    );
    require!(
        settlement.status == u8::from(ReceiptSettlementStatus::Recorded),
        ErrorCode::InvalidReceiptSettlementState
    );

    let provider = &mut ctx.accounts.provider_account;
    require!(
        provider.provider_wallet == settlement.provider_wallet,
        ErrorCode::InvalidProviderAccount
    );
    require_token_owner(
        &ctx.accounts.provider_destination_usdc,
        &settlement.provider_wallet,
    )?;
    require_token_mint(&ctx.accounts.provider_destination_usdc, &config.usdc_mint)?;

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.provider_pending_usdc_vault.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.provider_destination_usdc.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer,
        ),
        settlement.usdc_to_provider,
        ctx.accounts.usdc_mint.decimals,
    )?;

    provider.pending_provider_usdc = checked_sub_u64(
        provider.pending_provider_usdc,
        settlement.usdc_to_provider,
    )?;
    provider.unsettled_receipt_count = checked_sub_u64(provider.unsettled_receipt_count, 1)?;
    provider.updated_at = Clock::get()?.unix_timestamp;

    settlement.status = ReceiptSettlementStatus::FinalizedSettled.into();
    settlement.updated_at = Clock::get()?.unix_timestamp;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: RecordMiningFromReceiptArgs)]
pub struct RecordMiningFromReceipt<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    /// CHECK: singleton attestation config PDA signed by the configured attestation program
    #[account(
        seeds = [CONFIG_SEED],
        seeds::program = config.attestation_program,
        bump,
    )]
    pub attestation_config: Signer<'info>,
    #[account(mut)]
    pub payer_user: Signer<'info>,
    #[account(mut)]
    pub payer_usdc_token: Account<'info, TokenAccount>,
    /// CHECK: provider wallet identity
    pub provider_wallet: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [PROVIDER_SEED, provider_wallet.key().as_ref()],
        bump,
        constraint = provider_account.provider_wallet == provider_wallet.key() @ ErrorCode::InvalidProviderAccount,
    )]
    pub provider_account: Box<Account<'info, ProviderAccount>>,
    #[account(
        mut,
        seeds = [PROVIDER_REWARD_SEED, provider_wallet.key().as_ref()],
        bump,
    )]
    pub provider_reward_account: Box<Account<'info, RewardAccount>>,
    #[account(
        init_if_needed,
        payer = payer_user,
        space = REWARD_ACCOUNT_SPACE,
        seeds = [USER_REWARD_SEED, payer_user.key().as_ref()],
        bump,
    )]
    pub user_reward_account: Box<Account<'info, RewardAccount>>,
    #[account(
        init,
        payer = payer_user,
        space = RECEIPT_SETTLEMENT_SPACE,
        seeds = [RECEIPT_SETTLEMENT_SEED, attestation_receipt.key().as_ref()],
        bump,
    )]
    pub receipt_settlement: Box<Account<'info, ReceiptSettlement>>,
    /// CHECK: attestation receipt anchor only used as a unique key
    pub attestation_receipt: UncheckedAccount<'info>,
    #[account(
        mut,
        address = config.treasury_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub treasury_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = config.provider_pending_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub provider_pending_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettleFinalizedReceipt<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    /// CHECK: singleton attestation config PDA signed by the configured attestation program
    #[account(
        seeds = [CONFIG_SEED],
        seeds::program = config.attestation_program,
        bump,
    )]
    pub attestation_config: Signer<'info>,
    /// CHECK: attestation receipt identity
    pub attestation_receipt: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [RECEIPT_SETTLEMENT_SEED, attestation_receipt.key().as_ref()],
        bump,
    )]
    pub receipt_settlement: Box<Account<'info, ReceiptSettlement>>,
    #[account(
        mut,
        seeds = [PROVIDER_SEED, receipt_settlement.provider_wallet.as_ref()],
        bump,
        constraint = provider_account.provider_wallet == receipt_settlement.provider_wallet @ ErrorCode::InvalidProviderAccount,
    )]
    pub provider_account: Box<Account<'info, ProviderAccount>>,
    #[account(
        mut,
        address = config.provider_pending_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub provider_pending_usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub provider_destination_usdc: Account<'info, TokenAccount>,
    #[account(
        constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: PDA signer for the pending provider vault
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}
