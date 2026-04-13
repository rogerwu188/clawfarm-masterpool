use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase1ConfigParams {
    pub exchange_rate_claw_per_usdc_e6: u64,
    pub provider_stake_usdc: u64,
    pub provider_usdc_share_bps: u16,
    pub treasury_usdc_share_bps: u16,
    pub user_claw_share_bps: u16,
    pub provider_claw_share_bps: u16,
    pub lock_days: u16,
    pub provider_slash_claw_amount: u64,
    pub challenger_reward_bps: u16,
    pub burn_bps: u16,
    pub challenge_bond_claw_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProviderStatus {
    Active = 0,
    Exited = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RewardAccountKind {
    User = 0,
    Provider = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReceiptSettlementStatus {
    Recorded = 0,
    FinalizedSettled = 1,
    ChallengedReverted = 2,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChallengeBondStatus {
    Locked = 0,
    Returned = 1,
    Burned = 2,
}

impl From<ProviderStatus> for u8 {
    fn from(value: ProviderStatus) -> Self {
        value as u8
    }
}

impl From<RewardAccountKind> for u8 {
    fn from(value: RewardAccountKind) -> Self {
        value as u8
    }
}

impl From<ReceiptSettlementStatus> for u8 {
    fn from(value: ReceiptSettlementStatus) -> Self {
        value as u8
    }
}

impl From<ChallengeBondStatus> for u8 {
    fn from(value: ChallengeBondStatus) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for RewardAccountKind {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::User),
            1 => Ok(Self::Provider),
            _ => err!(crate::error::ErrorCode::InvalidRewardAccountKind),
        }
    }
}
