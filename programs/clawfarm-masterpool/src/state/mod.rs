use anchor_lang::Space;

pub mod accounts;
pub mod types;

pub use accounts::*;
pub use types::*;

pub const GLOBAL_CONFIG_SPACE: usize = 8 + GlobalConfig::INIT_SPACE;
pub const PROVIDER_ACCOUNT_SPACE: usize = 8 + ProviderAccount::INIT_SPACE;
pub const REWARD_ACCOUNT_SPACE: usize = 8 + RewardAccount::INIT_SPACE;
pub const RECEIPT_SETTLEMENT_SPACE: usize = 8 + ReceiptSettlement::INIT_SPACE;
pub const CHALLENGE_BOND_RECORD_SPACE: usize = 8 + ChallengeBondRecord::INIT_SPACE;
