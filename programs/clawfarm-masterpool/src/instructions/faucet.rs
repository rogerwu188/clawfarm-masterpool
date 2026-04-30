use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{
        CONFIG_SEED, DEFAULT_FAUCET_MAX_CLAW_GLOBAL_PER_DAY,
        DEFAULT_FAUCET_MAX_CLAW_PER_CLAIM, DEFAULT_FAUCET_MAX_CLAW_PER_WALLET_PER_DAY,
        DEFAULT_FAUCET_MAX_USDC_GLOBAL_PER_DAY, DEFAULT_FAUCET_MAX_USDC_PER_CLAIM,
        DEFAULT_FAUCET_MAX_USDC_PER_WALLET_PER_DAY, FAUCET_CLAW_VAULT_SEED,
        FAUCET_CONFIG_SEED, FAUCET_GLOBAL_SEED, FAUCET_USER_SEED, FAUCET_USDC_VAULT_SEED,
        POOL_AUTHORITY_SEED, SECONDS_PER_UTC_DAY,
    },
    error::ErrorCode,
    state::{
        FaucetClaimArgs, FaucetConfig, FaucetGlobalState, FaucetLimits, FaucetUserState,
        GlobalConfig, FAUCET_CONFIG_SPACE, FAUCET_GLOBAL_STATE_SPACE, FAUCET_USER_STATE_SPACE,
    },
    utils::{checked_add_u64, require_token_mint, require_token_owner, validate_faucet_limits},
};

pub fn initialize_faucet(ctx: Context<InitializeFaucet>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let limits = FaucetLimits {
        max_claw_per_claim: DEFAULT_FAUCET_MAX_CLAW_PER_CLAIM,
        max_usdc_per_claim: DEFAULT_FAUCET_MAX_USDC_PER_CLAIM,
        max_claw_per_wallet_per_day: DEFAULT_FAUCET_MAX_CLAW_PER_WALLET_PER_DAY,
        max_usdc_per_wallet_per_day: DEFAULT_FAUCET_MAX_USDC_PER_WALLET_PER_DAY,
        max_claw_global_per_day: DEFAULT_FAUCET_MAX_CLAW_GLOBAL_PER_DAY,
        max_usdc_global_per_day: DEFAULT_FAUCET_MAX_USDC_GLOBAL_PER_DAY,
    };
    validate_faucet_limits(&limits)?;

    let faucet_config = &mut ctx.accounts.faucet_config;
    faucet_config.admin_authority = ctx.accounts.admin_authority.key();
    faucet_config.enabled = false;
    faucet_config.faucet_claw_vault = ctx.accounts.faucet_claw_vault.key();
    faucet_config.faucet_usdc_vault = ctx.accounts.faucet_usdc_vault.key();
    apply_faucet_limits(faucet_config, &limits);
    faucet_config.created_at = now;
    faucet_config.updated_at = now;

    let global = &mut ctx.accounts.faucet_global_state;
    global.current_day_index = current_day_index(now)?;
    global.claw_claimed_today = 0;
    global.usdc_claimed_today = 0;
    global.updated_at = now;

    Ok(())
}

pub fn set_faucet_enabled(ctx: Context<SetFaucetEnabled>, enabled: bool) -> Result<()> {
    let faucet_config = &mut ctx.accounts.faucet_config;
    faucet_config.enabled = enabled;
    faucet_config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn update_faucet_limits(ctx: Context<UpdateFaucetLimits>, limits: FaucetLimits) -> Result<()> {
    validate_faucet_limits(&limits)?;
    let faucet_config = &mut ctx.accounts.faucet_config;
    apply_faucet_limits(faucet_config, &limits);
    faucet_config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn fund_faucet_claw(ctx: Context<FundFaucetClaw>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidFaucetAmount);
    require!(
        ctx.accounts.reward_vault.amount >= amount,
        ErrorCode::FaucetVaultInsufficientBalance
    );

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.reward_vault.to_account_info(),
                mint: ctx.accounts.claw_mint.to_account_info(),
                to: ctx.accounts.faucet_claw_vault.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.claw_mint.decimals,
    )?;

    ctx.accounts.faucet_config.updated_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn claim_faucet(ctx: Context<ClaimFaucet>, args: FaucetClaimArgs) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let day_index = current_day_index(now)?;
    let faucet_config = &ctx.accounts.faucet_config;
    let recipient = ctx.accounts.user.key();

    require!(faucet_config.enabled, ErrorCode::FaucetDisabled);
    require!(
        args.claw_amount > 0 || args.usdc_amount > 0,
        ErrorCode::InvalidFaucetAmount
    );
    require!(
        args.claw_amount <= faucet_config.max_claw_per_claim
            && args.usdc_amount <= faucet_config.max_usdc_per_claim,
        ErrorCode::FaucetClaimLimitExceeded
    );

    reset_global_if_needed(&mut ctx.accounts.faucet_global_state, day_index, now);
    initialize_or_reset_user_if_needed(
        &mut ctx.accounts.faucet_user_state,
        recipient,
        day_index,
        now,
    )?;

    let next_user_claw = checked_add_u64(
        ctx.accounts.faucet_user_state.claw_claimed_today,
        args.claw_amount,
    )?;
    let next_user_usdc = checked_add_u64(
        ctx.accounts.faucet_user_state.usdc_claimed_today,
        args.usdc_amount,
    )?;
    require!(
        next_user_claw <= faucet_config.max_claw_per_wallet_per_day
            && next_user_usdc <= faucet_config.max_usdc_per_wallet_per_day,
        ErrorCode::FaucetWalletDailyLimitExceeded
    );

    let next_global_claw = checked_add_u64(
        ctx.accounts.faucet_global_state.claw_claimed_today,
        args.claw_amount,
    )?;
    let next_global_usdc = checked_add_u64(
        ctx.accounts.faucet_global_state.usdc_claimed_today,
        args.usdc_amount,
    )?;
    require!(
        next_global_claw <= faucet_config.max_claw_global_per_day
            && next_global_usdc <= faucet_config.max_usdc_global_per_day,
        ErrorCode::FaucetGlobalDailyLimitExceeded
    );

    require_token_owner(&ctx.accounts.user_claw_token, &recipient)?;
    require_token_mint(&ctx.accounts.user_claw_token, &ctx.accounts.config.claw_mint)?;
    require_token_owner(&ctx.accounts.user_usdc_token, &recipient)?;
    require_token_mint(&ctx.accounts.user_usdc_token, &ctx.accounts.config.usdc_mint)?;
    require!(
        ctx.accounts.faucet_claw_vault.amount >= args.claw_amount
            && ctx.accounts.faucet_usdc_vault.amount >= args.usdc_amount,
        ErrorCode::FaucetVaultInsufficientBalance
    );

    ctx.accounts.faucet_user_state.claw_claimed_today = next_user_claw;
    ctx.accounts.faucet_user_state.usdc_claimed_today = next_user_usdc;
    ctx.accounts.faucet_user_state.updated_at = now;
    ctx.accounts.faucet_global_state.claw_claimed_today = next_global_claw;
    ctx.accounts.faucet_global_state.usdc_claimed_today = next_global_usdc;
    ctx.accounts.faucet_global_state.updated_at = now;

    let signer_seeds: &[&[u8]] = &[POOL_AUTHORITY_SEED, &[ctx.bumps.pool_authority]];
    let signer = &[signer_seeds];

    if args.claw_amount > 0 {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.faucet_claw_vault.to_account_info(),
                    mint: ctx.accounts.claw_mint.to_account_info(),
                    to: ctx.accounts.user_claw_token.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer,
            ),
            args.claw_amount,
            ctx.accounts.claw_mint.decimals,
        )?;
    }

    if args.usdc_amount > 0 {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.faucet_usdc_vault.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.user_usdc_token.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer,
            ),
            args.usdc_amount,
            ctx.accounts.usdc_mint.decimals,
        )?;
    }

    Ok(())
}

pub fn current_day_index(unix_timestamp: i64) -> Result<i64> {
    unix_timestamp
        .checked_div(SECONDS_PER_UTC_DAY)
        .ok_or_else(|| error!(ErrorCode::MathOverflow))
}

fn apply_faucet_limits(config: &mut FaucetConfig, limits: &FaucetLimits) {
    config.max_claw_per_claim = limits.max_claw_per_claim;
    config.max_usdc_per_claim = limits.max_usdc_per_claim;
    config.max_claw_per_wallet_per_day = limits.max_claw_per_wallet_per_day;
    config.max_usdc_per_wallet_per_day = limits.max_usdc_per_wallet_per_day;
    config.max_claw_global_per_day = limits.max_claw_global_per_day;
    config.max_usdc_global_per_day = limits.max_usdc_global_per_day;
}

pub(crate) fn reset_global_if_needed(global: &mut FaucetGlobalState, day_index: i64, now: i64) {
    if global.current_day_index != day_index {
        global.current_day_index = day_index;
        global.claw_claimed_today = 0;
        global.usdc_claimed_today = 0;
        global.updated_at = now;
    }
}

pub(crate) fn initialize_or_reset_user_if_needed(
    user_state: &mut FaucetUserState,
    owner: Pubkey,
    day_index: i64,
    now: i64,
) -> Result<()> {
    if user_state.owner == Pubkey::default() {
        user_state.owner = owner;
        user_state.current_day_index = day_index;
        user_state.claw_claimed_today = 0;
        user_state.usdc_claimed_today = 0;
        user_state.created_at = now;
        user_state.updated_at = now;
        return Ok(());
    }

    require!(user_state.owner == owner, ErrorCode::InvalidFaucetUserState);
    if user_state.current_day_index != day_index {
        user_state.current_day_index = day_index;
        user_state.claw_claimed_today = 0;
        user_state.usdc_claimed_today = 0;
        user_state.updated_at = now;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anchor_lang::prelude::*;

    use super::{initialize_or_reset_user_if_needed, reset_global_if_needed};
    use crate::state::{FaucetGlobalState, FaucetUserState};

    #[test]
    fn resets_global_state_when_day_changes() {
        let mut global = FaucetGlobalState {
            current_day_index: 10,
            claw_claimed_today: 7,
            usdc_claimed_today: 8,
            updated_at: 1,
        };
        reset_global_if_needed(&mut global, 11, 99);
        assert_eq!(global.current_day_index, 11);
        assert_eq!(global.claw_claimed_today, 0);
        assert_eq!(global.usdc_claimed_today, 0);
        assert_eq!(global.updated_at, 99);
    }

    #[test]
    fn keeps_global_state_when_day_matches() {
        let mut global = FaucetGlobalState {
            current_day_index: 10,
            claw_claimed_today: 7,
            usdc_claimed_today: 8,
            updated_at: 1,
        };
        reset_global_if_needed(&mut global, 10, 99);
        assert_eq!(global.claw_claimed_today, 7);
        assert_eq!(global.usdc_claimed_today, 8);
        assert_eq!(global.updated_at, 1);
    }

    #[test]
    fn initializes_empty_user_state_once() {
        let owner = Pubkey::new_unique();
        let mut user = FaucetUserState {
            owner: Pubkey::default(),
            current_day_index: 0,
            claw_claimed_today: 0,
            usdc_claimed_today: 0,
            created_at: 0,
            updated_at: 0,
        };
        initialize_or_reset_user_if_needed(&mut user, owner, 20, 100).unwrap();
        assert_eq!(user.owner, owner);
        assert_eq!(user.current_day_index, 20);
        assert_eq!(user.created_at, 100);
    }

    #[test]
    fn resets_user_state_when_day_changes() {
        let owner = Pubkey::new_unique();
        let mut user = FaucetUserState {
            owner,
            current_day_index: 20,
            claw_claimed_today: 5,
            usdc_claimed_today: 6,
            created_at: 10,
            updated_at: 11,
        };
        initialize_or_reset_user_if_needed(&mut user, owner, 21, 100).unwrap();
        assert_eq!(user.current_day_index, 21);
        assert_eq!(user.claw_claimed_today, 0);
        assert_eq!(user.usdc_claimed_today, 0);
        assert_eq!(user.created_at, 10);
        assert_eq!(user.updated_at, 100);
    }
}

#[derive(Accounts)]
pub struct InitializeFaucet<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
        has_one = claw_mint @ ErrorCode::InvalidClawMint,
        has_one = usdc_mint @ ErrorCode::InvalidUsdcMint,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(init, payer = payer, space = FAUCET_CONFIG_SPACE, seeds = [FAUCET_CONFIG_SEED], bump)]
    pub faucet_config: Box<Account<'info, FaucetConfig>>,
    #[account(init, payer = payer, space = FAUCET_GLOBAL_STATE_SPACE, seeds = [FAUCET_GLOBAL_SEED], bump)]
    pub faucet_global_state: Box<Account<'info, FaucetGlobalState>>,
    #[account(
        init,
        payer = payer,
        token::mint = claw_mint,
        token::authority = pool_authority,
        seeds = [FAUCET_CLAW_VAULT_SEED],
        bump,
    )]
    pub faucet_claw_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = payer,
        token::mint = usdc_mint,
        token::authority = pool_authority,
        seeds = [FAUCET_USDC_VAULT_SEED],
        bump,
    )]
    pub faucet_usdc_vault: Box<Account<'info, TokenAccount>>,
    #[account(constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint)]
    pub claw_mint: Box<Account<'info, Mint>>,
    #[account(constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer for program-owned vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub admin_authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SetFaucetEnabled<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,
    pub admin_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFaucetLimits<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,
    pub admin_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct FundFaucetClaw<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
        has_one = claw_mint @ ErrorCode::InvalidClawMint,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump,
        has_one = admin_authority @ ErrorCode::UnauthorizedAdmin,
        has_one = faucet_claw_vault @ ErrorCode::InvalidFaucetVault,
    )]
    pub faucet_config: Box<Account<'info, FaucetConfig>>,
    #[account(
        mut,
        address = config.reward_vault @ ErrorCode::InvalidVaultAccount,
        constraint = reward_vault.mint == config.claw_mint @ ErrorCode::InvalidTokenMint,
        constraint = reward_vault.owner == pool_authority.key() @ ErrorCode::InvalidVaultAccount,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = faucet_config.faucet_claw_vault @ ErrorCode::InvalidFaucetVault,
        constraint = faucet_claw_vault.mint == config.claw_mint @ ErrorCode::InvalidTokenMint,
        constraint = faucet_claw_vault.owner == pool_authority.key() @ ErrorCode::InvalidFaucetVault,
    )]
    pub faucet_claw_vault: Box<Account<'info, TokenAccount>>,
    #[account(constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint)]
    pub claw_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer for reward and faucet vaults
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    pub admin_authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimFaucet<'info> {
    #[account(seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump,
        has_one = faucet_claw_vault @ ErrorCode::InvalidFaucetVault,
        has_one = faucet_usdc_vault @ ErrorCode::InvalidFaucetVault,
    )]
    pub faucet_config: Box<Account<'info, FaucetConfig>>,
    #[account(mut, seeds = [FAUCET_GLOBAL_SEED], bump)]
    pub faucet_global_state: Box<Account<'info, FaucetGlobalState>>,
    #[account(
        init_if_needed,
        payer = payer,
        space = FAUCET_USER_STATE_SPACE,
        seeds = [FAUCET_USER_SEED, user.key().as_ref()],
        bump,
    )]
    pub faucet_user_state: Box<Account<'info, FaucetUserState>>,
    #[account(
        mut,
        address = faucet_config.faucet_claw_vault @ ErrorCode::InvalidFaucetVault,
        constraint = faucet_claw_vault.mint == config.claw_mint @ ErrorCode::InvalidTokenMint,
        constraint = faucet_claw_vault.owner == pool_authority.key() @ ErrorCode::InvalidFaucetVault,
    )]
    pub faucet_claw_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = faucet_config.faucet_usdc_vault @ ErrorCode::InvalidFaucetVault,
        constraint = faucet_usdc_vault.mint == config.usdc_mint @ ErrorCode::InvalidTokenMint,
        constraint = faucet_usdc_vault.owner == pool_authority.key() @ ErrorCode::InvalidFaucetVault,
    )]
    pub faucet_usdc_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_claw_token: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_usdc_token: Box<Account<'info, TokenAccount>>,
    #[account(constraint = claw_mint.key() == config.claw_mint @ ErrorCode::InvalidClawMint)]
    pub claw_mint: Box<Account<'info, Mint>>,
    #[account(constraint = usdc_mint.key() == config.usdc_mint @ ErrorCode::InvalidUsdcMint)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: faucet source/token authority PDA for faucet vault transfers
    #[account(seeds = [POOL_AUTHORITY_SEED], bump)]
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: recipient wallet public key; does not sign faucet claims
    pub user: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
