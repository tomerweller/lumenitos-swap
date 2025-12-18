use soroban_sdk::{contracttype, Address};

/// Position key for pool-level tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionKey {
    pub owner: Address,
    pub tick_lower: i32,
    pub tick_upper: i32,
}

/// Position info stored in pool contract
#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct PositionInfo {
    /// Liquidity in this position
    pub liquidity: u128,
    /// Fee growth inside at last update (token0)
    pub fee_growth_inside_0_last_x128: u128,
    /// Fee growth inside at last update (token1)
    pub fee_growth_inside_1_last_x128: u128,
    /// Uncollected token0 fees
    pub tokens_owed_0: u128,
    /// Uncollected token1 fees
    pub tokens_owed_1: u128,
}

impl PositionInfo {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Position data for Position Manager (NFT metadata)
#[contracttype]
#[derive(Clone, Debug)]
pub struct PositionData {
    /// Pool contract address
    pub pool: Address,
    /// Lower tick boundary
    pub tick_lower: i32,
    /// Upper tick boundary
    pub tick_upper: i32,
    /// Liquidity amount
    pub liquidity: u128,
    /// Fee growth inside at last action (token0)
    pub fee_growth_inside_0_last_x128: u128,
    /// Fee growth inside at last action (token1)
    pub fee_growth_inside_1_last_x128: u128,
    /// Tokens owed (token0)
    pub tokens_owed_0: u128,
    /// Tokens owed (token1)
    pub tokens_owed_1: u128,
}
