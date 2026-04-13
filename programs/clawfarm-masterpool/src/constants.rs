pub const CONFIG_SEED: &[u8] = b"config";
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool_authority";
pub const REWARD_VAULT_SEED: &[u8] = b"reward_vault";
pub const CHALLENGE_BOND_VAULT_SEED: &[u8] = b"challenge_bond_vault";
pub const TREASURY_USDC_VAULT_SEED: &[u8] = b"treasury_usdc_vault";
pub const PROVIDER_STAKE_USDC_VAULT_SEED: &[u8] = b"provider_stake_usdc_vault";
pub const PROVIDER_PENDING_USDC_VAULT_SEED: &[u8] = b"provider_pending_usdc_vault";
pub const PROVIDER_SEED: &[u8] = b"provider";
pub const PROVIDER_REWARD_SEED: &[u8] = b"provider_reward";
pub const USER_REWARD_SEED: &[u8] = b"user_reward";
pub const RECEIPT_SETTLEMENT_SEED: &[u8] = b"receipt_settlement";
pub const CHALLENGE_BOND_RECORD_SEED: &[u8] = b"challenge_bond_record";

pub const BPS_SCALE: u16 = 1_000;
pub const RATE_SCALE: u64 = 1_000_000;
pub const CLAW_DECIMALS: u8 = 6;
pub const USDC_DECIMALS: u8 = 6;
pub const GENESIS_TOTAL_SUPPLY: u64 = 1_000_000_000 * RATE_SCALE;

pub const DEFAULT_EXCHANGE_RATE_CLAW_PER_USDC_E6: u64 = RATE_SCALE;
pub const DEFAULT_PROVIDER_STAKE_USDC: u64 = 100 * RATE_SCALE;
pub const DEFAULT_PROVIDER_USDC_SHARE_BPS: u16 = 300;
pub const DEFAULT_TREASURY_USDC_SHARE_BPS: u16 = 700;
pub const DEFAULT_USER_CLAW_SHARE_BPS: u16 = 300;
pub const DEFAULT_PROVIDER_CLAW_SHARE_BPS: u16 = 700;
pub const DEFAULT_LOCK_DAYS: u16 = 180;
pub const DEFAULT_PROVIDER_SLASH_CLAW_AMOUNT: u64 = RATE_SCALE;
pub const DEFAULT_CHALLENGER_REWARD_BPS: u16 = 700;
pub const DEFAULT_BURN_BPS: u16 = 300;
pub const DEFAULT_CHALLENGE_BOND_CLAW_AMOUNT: u64 = 10 * RATE_SCALE;

pub const ATTESTATION_RECEIPT_STATUS_FINALIZED: u8 = 2;
pub const ATTESTATION_RESOLUTION_ACCEPTED: u8 = 1;
pub const ATTESTATION_RESOLUTION_REJECTED: u8 = 2;
pub const ATTESTATION_RESOLUTION_RECEIPT_INVALIDATED: u8 = 3;
pub const ATTESTATION_RESOLUTION_SIGNER_REVOKED: u8 = 4;
