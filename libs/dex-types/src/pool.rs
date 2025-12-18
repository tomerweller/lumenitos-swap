use soroban_sdk::{contracttype, Address};

/// Current pool state - stored in Instance storage for frequent access
#[contracttype]
#[derive(Clone, Debug)]
pub struct PoolState {
    /// Current sqrt(price) as Q64.96
    pub sqrt_price_x96: u128,
    /// Current tick index
    pub tick: i32,
    /// Total liquidity currently in range
    pub liquidity: u128,
    /// Fee growth global for token0 (Q128.128)
    pub fee_growth_global_0_x128: u128,
    /// Fee growth global for token1 (Q128.128)
    pub fee_growth_global_1_x128: u128,
    /// Protocol fees accumulated for token0
    pub protocol_fees_0: i128,
    /// Protocol fees accumulated for token1
    pub protocol_fees_1: i128,
}

impl PoolState {
    pub fn new(sqrt_price_x96: u128, tick: i32) -> Self {
        Self {
            sqrt_price_x96,
            tick,
            liquidity: 0,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        }
    }
}

/// Pool configuration - immutable after creation
#[contracttype]
#[derive(Clone, Debug)]
pub struct PoolConfig {
    /// Factory contract address
    pub factory: Address,
    /// Token0 address (lower address)
    pub token0: Address,
    /// Token1 address (higher address)
    pub token1: Address,
    /// Fee tier in hundredths of bps
    pub fee: u32,
    /// Tick spacing for this pool
    pub tick_spacing: i32,
    /// Maximum liquidity per tick
    pub max_liquidity_per_tick: u128,
}
