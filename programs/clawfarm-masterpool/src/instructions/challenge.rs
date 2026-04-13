use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{
        ATTESTATION_RESOLUTION_ACCEPTED, ATTESTATION_RESOLUTION_RECEIPT_INVALIDATED,
        ATTESTATION_RESOLUTION_REJECTED, ATTESTATION_RESOLUTION_SIGNER_REVOKED,
        CHALLENGE_BOND_RECORD_SEED, CONFIG_SEED, POOL_AUTHORITY_SEED, PROVIDER_SEED,
        RECEIPT_SETTLEMENT_SEED,
    },
    error::ErrorCode,
    state::{
        ChallengeBondRecord, ChallengeBondStatus, GlobalConfig, ProviderAccount, ReceiptSettlement,
        ReceiptSettlementStatus, CHALLENGE_BOND_RECORD_SPACE,
    },
    utils::{
        calculate_bps_amount, checked_add_u64, checked_sub_i64, checked_sub_u64,
        require_attestation_caller, require_token_mint, require_token_mint_info,
        require_token_owner, require_token_owner_info,
    },
};

pub fn record_challenge_bond(ctx: Context<RecordChallengeBond>) -> Result<()> {
    let config = &ctx.accounts.config;
    require_attestation_caller(config, &ctx.accounts.attestation_config.to_account_info())?;
    require!(!config.pause_challenge_processing, ErrorCode::ChallengeProcessingPaused);

    let settlement = &ctx.accounts.receipt_settlement;
    require!(
        settlement.status == u8::from(ReceiptSettlementStatus::Recorded),
        ErrorCode::InvalidReceiptSettlementState
    );

    require_token_owner(
        &ctx.accounts.challenger_claw_token,
        &ctx.accounts.challenger.key(),
    )?;
    require_token_mint(&ctx.accounts.challenger_claw_token, &config.claw_mint)?;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.challenger_claw_token.to_account_info(),
                mint: ctx.accounts.claw_mint.to_account_info(),
                to: ctx.accounts.challenge_bond_vault.to_account_info(),
                authority: ctx.accounts.challenger.to_account_info(),
            },
        ),
        config.challenge_bond_claw_amount,
        ctx.accounts.claw_mint.decimals,
    )?;

    let challenger_reward_amount = calculate_bps_amount(
        config.provider_slash_claw_amount,
        config.challenger_reward_bps,
    )?;
    let burn_amount = checked_sub_u64(
        config.provider_slash_claw_amount,
        challenger_reward_amount,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let record = &mut ctx.accounts.challenge_bond_record;
    record.attestation_challenge = ctx.accounts.attestation_challenge.key();
    record.attestation_receipt = ctx.accounts.attestation_receipt.key();
    record.challenger = ctx.accounts.challenger.key();
    record.payer_user = settlement.payer_user;
    record.provider_wallet = settlement.provider_wallet;
    record.bond_amount = config.challenge_bond_claw_amount;
    record.slash_claw_amount_snapshot = config.provider_slash_claw_amount;
    record.challenger_reward_bps_snapshot = config.challenger_reward_bps;
    record.burn_bps_snapshot = config.burn_bps;
    record.challenger_reward_amount = challenger_reward_amount;
    record.burn_amount = burn_amount;
    record.status = ChallengeBondStatus::Locked.into();
    record.created_at = now;
    record.updated_at = now;

    let provider = &mut ctx.accounts.provider_account;
    provider.unresolved_challenge_count = checked_add_u64(provider.unresolved_challenge_count, 1)?;
    provider.updated_at = now;

    Ok(())
}

pub fn resolve_challenge_economics(
    ctx: Context<ResolveChallengeEconomics>,
    resolution_code: u8,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require_attestation_caller(config, &ctx.accounts.attestation_config.to_account_info())?;
    require!(!config.pause_challenge_processing, ErrorCode::ChallengeProcessingPaused);

    let settlement = &mut ctx.accounts.receipt_settlement;
    require!(
        settlement.status == u8::from(ReceiptSettlementStatus::Recorded),
        ErrorCode::InvalidReceiptSettlementState
    );
    require!(
        settlement.attestation_receipt == ctx.accounts.attestation_receipt.key(),
        ErrorCode::InvalidReceiptSettlement
    );

    let record = &mut ctx.accounts.challenge_bond_record;
    require!(
        record.attestation_challenge == ctx.accounts.attestation_challenge.key()
            && record.attestation_receipt == settlement.attestation_receipt,
        ErrorCode::InvalidChallengeBondRecord
    );
    require!(
        record.status == u8::from(ChallengeBondStatus::Locked),
        ErrorCode::InvalidChallengeBondState
    );

    let provider = &mut ctx.accounts.provider_account;
    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    match resolution_code {
        ATTESTATION_RESOLUTION_REJECTED => {
            token::burn(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.claw_mint.to_account_info(),
                        from: ctx.accounts.challenge_bond_vault.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer,
                ),
                record.bond_amount,
            )?;

            record.status = ChallengeBondStatus::Burned.into();
        }
        ATTESTATION_RESOLUTION_ACCEPTED
        | ATTESTATION_RESOLUTION_RECEIPT_INVALIDATED
        | ATTESTATION_RESOLUTION_SIGNER_REVOKED => {
            require_token_owner_info(&ctx.accounts.payer_usdc_token, &settlement.payer_user)?;
            require_token_mint_info(&ctx.accounts.payer_usdc_token, &config.usdc_mint)?;
            require_token_owner_info(
                &ctx.accounts.challenger_claw_token,
                &record.challenger,
            )?;
            require_token_mint_info(&ctx.accounts.challenger_claw_token, &config.claw_mint)?;

            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.challenge_bond_vault.to_account_info(),
                        mint: ctx.accounts.claw_mint.to_account_info(),
                        to: ctx.accounts.challenger_claw_token.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer,
                ),
                record.bond_amount,
                crate::constants::CLAW_DECIMALS,
            )?;
            if record.challenger_reward_amount > 0 {
                token::transfer_checked(
                    CpiContext::new_with_signer(
                        ctx.accounts.token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.reward_vault.to_account_info(),
                            mint: ctx.accounts.claw_mint.to_account_info(),
                            to: ctx.accounts.challenger_claw_token.to_account_info(),
                            authority: ctx.accounts.pool_authority.to_account_info(),
                        },
                        signer,
                    ),
                    record.challenger_reward_amount,
                    crate::constants::CLAW_DECIMALS,
                )?;
            }
            if record.burn_amount > 0 {
                token::burn(
                    CpiContext::new_with_signer(
                        ctx.accounts.token_program.to_account_info(),
                        Burn {
                            mint: ctx.accounts.claw_mint.to_account_info(),
                            from: ctx.accounts.reward_vault.to_account_info(),
                            authority: ctx.accounts.pool_authority.to_account_info(),
                        },
                        signer,
                    ),
                    record.burn_amount,
                )?;
            }
            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.provider_pending_usdc_vault.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.payer_usdc_token.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer,
                ),
                settlement.usdc_to_provider,
                crate::constants::USDC_DECIMALS,
            )?;

            provider.pending_provider_usdc = checked_sub_u64(
                provider.pending_provider_usdc,
                settlement.usdc_to_provider,
            )?;
            provider.unsettled_receipt_count = checked_sub_u64(provider.unsettled_receipt_count, 1)?;
            provider.claw_net_position = checked_sub_i64(
                provider.claw_net_position,
                record.slash_claw_amount_snapshot as i64,
            )?;
            settlement.status = ReceiptSettlementStatus::ChallengedReverted.into();
            record.status = ChallengeBondStatus::Returned.into();
        }
        _ => return err!(ErrorCode::InvalidChallengeResolution),
    }

    provider.unresolved_challenge_count = checked_sub_u64(provider.unresolved_challenge_count, 1)?;
    let now = Clock::get()?.unix_timestamp;
    provider.updated_at = now;
    settlement.updated_at = now;
    record.updated_at = now;

    Ok(())
}

#[derive(Accounts)]
pub struct RecordChallengeBond<'info> {
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
    pub challenger: Signer<'info>,
    #[account(mut)]
    pub challenger_claw_token: Account<'info, TokenAccount>,
    /// CHECK: attestation receipt identity
    pub attestation_receipt: UncheckedAccount<'info>,
    /// CHECK: attestation challenge identity
    pub attestation_challenge: UncheckedAccount<'info>,
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
        init,
        payer = challenger,
        space = CHALLENGE_BOND_RECORD_SPACE,
        seeds = [CHALLENGE_BOND_RECORD_SEED, attestation_challenge.key().as_ref()],
        bump,
    )]
    pub challenge_bond_record: Box<Account<'info, ChallengeBondRecord>>,
    #[account(
        mut,
        address = config.challenge_bond_vault @ ErrorCode::InvalidVaultAccount,
    )]
    /// CHECK: masterpool validates the configured vault binding
    pub challenge_bond_vault: UncheckedAccount<'info>,
    #[account(
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint,
    )]
    pub claw_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveChallengeEconomics<'info> {
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
    /// CHECK: attestation challenge identity
    pub attestation_challenge: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [RECEIPT_SETTLEMENT_SEED, attestation_receipt.key().as_ref()],
        bump,
    )]
    pub receipt_settlement: Box<Account<'info, ReceiptSettlement>>,
    #[account(
        mut,
        seeds = [CHALLENGE_BOND_RECORD_SEED, attestation_challenge.key().as_ref()],
        bump,
    )]
    pub challenge_bond_record: Box<Account<'info, ChallengeBondRecord>>,
    #[account(
        mut,
        seeds = [PROVIDER_SEED, receipt_settlement.provider_wallet.as_ref()],
        bump,
        constraint = provider_account.provider_wallet == receipt_settlement.provider_wallet @ ErrorCode::InvalidProviderAccount,
    )]
    pub provider_account: Box<Account<'info, ProviderAccount>>,
    #[account(
        mut,
        address = config.challenge_bond_vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub challenge_bond_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = config.reward_vault @ ErrorCode::InvalidVaultAccount,
    )]
    /// CHECK: masterpool validates the configured vault binding
    pub reward_vault: UncheckedAccount<'info>,
    #[account(
        mut,
        address = config.provider_pending_usdc_vault @ ErrorCode::InvalidVaultAccount,
    )]
    /// CHECK: masterpool validates the configured vault binding
    pub provider_pending_usdc_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated at runtime via token accessors
    pub challenger_claw_token: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: validated at runtime via token accessors
    pub payer_usdc_token: UncheckedAccount<'info>,
    #[account(
        constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint,
    )]
    #[account(mut)]
    /// CHECK: masterpool validates the configured mint binding
    pub claw_mint: UncheckedAccount<'info>,
    #[account(
        constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    /// CHECK: masterpool validates the configured mint binding
    pub usdc_mint: UncheckedAccount<'info>,
    /// CHECK: PDA signer for all masterpool vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}
