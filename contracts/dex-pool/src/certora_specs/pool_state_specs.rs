// ============================================================================
// POOL STATE SPECIFICATIONS
// ============================================================================
//
// These specifications verify pool state invariants by calling actual
// contract functions and reading real storage.
//
// ============================================================================

#[cfg(feature = "certora")]
use soroban_sdk::{Address, Env};

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume};

#[cfg(feature = "certora")]
use crate::DexPool;

/// RULE: After initialization, sqrt_price matches the input
#[cfg(feature = "certora")]
#[rule]
pub fn init_sets_correct_price(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    fee: u32,
    tick_spacing: i32,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    // Preconditions for valid initialization
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);
    cvlr_assume!(fee <= 1_000_000);

    // Call initialize
    DexPool::initialize(
        env.clone(),
        factory,
        token0,
        token1,
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Verify state matches input
    let state = DexPool::get_state(env.clone());
    cvlr_assert!(state.sqrt_price_x96 == sqrt_price_x96);
}

/// RULE: After initialization, tick is consistent with sqrt_price
#[cfg(feature = "certora")]
#[rule]
pub fn init_tick_consistent_with_price(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    fee: u32,
    tick_spacing: i32,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);
    cvlr_assume!(fee <= 1_000_000);

    DexPool::initialize(
        env.clone(),
        factory,
        token0,
        token1,
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let state = DexPool::get_state(env.clone());

    // The tick should be what get_tick_at_sqrt_ratio returns for this price
    let expected_tick = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_price_x96);
    cvlr_assert!(state.tick == expected_tick);
}

/// RULE: After initialization, liquidity is zero
#[cfg(feature = "certora")]
#[rule]
pub fn init_liquidity_is_zero(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    fee: u32,
    tick_spacing: i32,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);

    DexPool::initialize(
        env.clone(),
        factory,
        token0,
        token1,
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let state = DexPool::get_state(env.clone());
    cvlr_assert!(state.liquidity == 0);
}

/// RULE: After initialization, fee growth globals are zero
#[cfg(feature = "certora")]
#[rule]
pub fn init_fee_growth_is_zero(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    fee: u32,
    tick_spacing: i32,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);

    DexPool::initialize(
        env.clone(),
        factory,
        token0,
        token1,
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let state = DexPool::get_state(env.clone());
    cvlr_assert!(state.fee_growth_global_0_x128 == 0);
    cvlr_assert!(state.fee_growth_global_1_x128 == 0);
}

/// RULE: Config stores correct tick_spacing
#[cfg(feature = "certora")]
#[rule]
pub fn init_stores_tick_spacing(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    fee: u32,
    tick_spacing: i32,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);

    DexPool::initialize(
        env.clone(),
        factory,
        token0,
        token1,
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let config = DexPool::get_config(env.clone());
    cvlr_assert!(config.tick_spacing == tick_spacing);
    cvlr_assert!(config.fee == fee);
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use dex_types::{PoolState, Q96, MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    fn create_valid_state() -> PoolState {
        PoolState {
            sqrt_price_x96: Q96,
            tick: 0,
            liquidity: 1000000,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        }
    }

    #[test]
    fn test_price_bounds() {
        let state = create_valid_state();
        assert!(state.sqrt_price_x96 > MIN_SQRT_RATIO);
        assert!(state.sqrt_price_x96 < MAX_SQRT_RATIO);
    }

    #[test]
    fn test_tick_bounds() {
        let state = create_valid_state();
        assert!(state.tick >= MIN_TICK);
        assert!(state.tick <= MAX_TICK);
    }

    #[test]
    fn test_fee_growth_starts_zero() {
        let state = PoolState::new(Q96, 0);
        assert_eq!(state.fee_growth_global_0_x128, 0);
        assert_eq!(state.fee_growth_global_1_x128, 0);
    }
}
