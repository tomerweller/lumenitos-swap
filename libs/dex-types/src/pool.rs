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

// ============================================================================
// SWAP COMPUTATION TYPES - For Formal Verification
// These types separate pure computation from side effects
// ============================================================================

/// Parameters for a swap operation (input to pure computation)
#[derive(Clone, Debug)]
pub struct SwapParams {
    /// True if swapping token0 for token1
    pub zero_for_one: bool,
    /// Positive for exact input, negative for exact output
    pub amount_specified: i128,
    /// Price limit for the swap
    pub sqrt_price_limit_x96: u128,
}

/// Intermediate state during swap computation (pure, no storage)
#[derive(Clone, Debug)]
pub struct SwapState {
    /// Amount remaining to be swapped
    pub amount_remaining: i128,
    /// Cumulative amount of the other token
    pub amount_calculated: i128,
    /// Current sqrt price
    pub sqrt_price_x96: u128,
    /// Current tick
    pub tick: i32,
    /// Current liquidity
    pub liquidity: u128,
    /// Fee growth accumulator for the input token
    pub fee_growth_global_x128: u128,
}

/// Result of a single swap step (pure computation)
#[derive(Clone, Debug)]
pub struct SwapStepResult {
    /// New sqrt price after this step
    pub sqrt_price_next_x96: u128,
    /// Amount of input token consumed
    pub amount_in: u128,
    /// Amount of output token produced
    pub amount_out: u128,
    /// Fee amount taken
    pub fee_amount: u128,
}

/// Information about a tick crossing during swap
#[derive(Clone, Debug)]
pub struct TickCrossing {
    /// The tick that was crossed
    pub tick: i32,
    /// Liquidity delta to apply (already adjusted for direction)
    pub liquidity_delta: i128,
}

/// Complete result of pure swap computation
#[derive(Clone, Debug)]
pub struct SwapComputation {
    /// Final amount of token0 (positive = user pays, negative = user receives)
    pub amount0: i128,
    /// Final amount of token1 (positive = user pays, negative = user receives)
    pub amount1: i128,
    /// Final sqrt price after swap
    pub sqrt_price_x96: u128,
    /// Final tick after swap
    pub tick: i32,
    /// Final liquidity after swap
    pub liquidity: u128,
    /// Updated fee growth global for input token
    pub fee_growth_global_x128: u128,
    /// Whether the fee growth is for token0 (true) or token1 (false)
    pub fee_growth_is_token0: bool,
    /// Ticks that were crossed (need storage updates)
    pub ticks_crossed: u32,
}
