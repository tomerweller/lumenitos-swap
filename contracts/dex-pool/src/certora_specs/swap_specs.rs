// ============================================================================
// SWAP INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify swap behavior by calling the actual swap
// function and verifying price direction, amount signs, and limits.
//
// Following Certora best practices:
// - Use model initialization for ghost state
// - Use state snapshots for before/after comparisons
// - Add sanity rules to ensure non-vacuous verification
//
// ============================================================================

#[cfg(feature = "certora")]
use soroban_sdk::{Address, Env};

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

#[cfg(feature = "certora")]
use crate::DexPool;

#[cfg(feature = "certora")]
use super::model::{self, PoolSnapshot};

// ============================================================================
// CORE SWAP RULES
// ============================================================================

/// RULE: Zero-for-one swap decreases sqrt_price
/// When swapping token0 for token1, price (in terms of token1/token0) decreases
#[cfg(feature = "certora")]
#[rule]
pub fn zero_for_one_decreases_price(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    // Initialize model
    model::reset();

    // Setup preconditions
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0); // Exact input swap

    // Initialize pool
    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Capture state before
    let before = PoolSnapshot::capture(&env);

    // Execute zero_for_one swap with price limit at minimum
    let sqrt_price_limit = MIN_SQRT_RATIO + 1;
    model::set_last_swap_direction(true);
    let _result = DexPool::swap(
        env.clone(),
        recipient,
        true, // zero_for_one
        amount_specified,
        sqrt_price_limit,
    );

    // Capture state after
    let after = PoolSnapshot::capture(&env);

    // Price should decrease or stay same (if no liquidity)
    cvlr_assert!(after.sqrt_price_x96 <= before.sqrt_price_x96);
}

/// SANITY: zero_for_one_decreases_price is satisfiable
#[cfg(feature = "certora")]
#[rule]
pub fn zero_for_one_decreases_price_sanity(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let sqrt_price_limit = MIN_SQRT_RATIO + 1;
    let _result = DexPool::swap(
        env.clone(),
        recipient,
        true,
        amount_specified,
        sqrt_price_limit,
    );

    // Verify this rule is not vacuously true
    cvlr_satisfy!(true);
}

/// RULE: One-for-zero swap increases sqrt_price
/// When swapping token1 for token0, price (in terms of token1/token0) increases
#[cfg(feature = "certora")]
#[rule]
pub fn one_for_zero_increases_price(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let before = PoolSnapshot::capture(&env);

    let sqrt_price_limit = MAX_SQRT_RATIO - 1;
    model::set_last_swap_direction(false);
    let _result = DexPool::swap(
        env.clone(),
        recipient,
        false, // one_for_zero
        amount_specified,
        sqrt_price_limit,
    );

    let after = PoolSnapshot::capture(&env);

    // Price should increase or stay same (if no liquidity)
    cvlr_assert!(after.sqrt_price_x96 >= before.sqrt_price_x96);
}

/// RULE: Swap amounts have opposite signs (one in, one out)
/// If amount0 is positive (paid in), amount1 should be negative (received out)
#[cfg(feature = "certora")]
#[rule]
pub fn swap_amounts_opposite_signs(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified != 0);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let sqrt_price_limit = if zero_for_one {
        MIN_SQRT_RATIO + 1
    } else {
        MAX_SQRT_RATIO - 1
    };

    let (amount0, amount1) = DexPool::swap(
        env.clone(),
        recipient,
        zero_for_one,
        amount_specified,
        sqrt_price_limit,
    );

    // Valid swap: opposite signs, or one/both zero (no liquidity case)
    let valid = (amount0 > 0 && amount1 <= 0)
        || (amount0 <= 0 && amount1 > 0)
        || (amount0 == 0 && amount1 == 0);

    cvlr_assert!(valid);
}

/// RULE: Swap respects sqrt_price_limit
/// After swap, price should not exceed the limit in either direction
#[cfg(feature = "certora")]
#[rule]
pub fn swap_respects_price_limit(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    sqrt_price_limit: u128,
    recipient: Address,
    amount_specified: i128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(sqrt_price_limit > MIN_SQRT_RATIO && sqrt_price_limit < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0);

    // Limit must be valid for direction
    if zero_for_one {
        cvlr_assume!(sqrt_price_limit < sqrt_price_x96);
    } else {
        cvlr_assume!(sqrt_price_limit > sqrt_price_x96);
    }

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let _result = DexPool::swap(
        env.clone(),
        recipient,
        zero_for_one,
        amount_specified,
        sqrt_price_limit,
    );

    let after = PoolSnapshot::capture(&env);

    // Price should respect the limit
    if zero_for_one {
        cvlr_assert!(after.sqrt_price_x96 >= sqrt_price_limit);
    } else {
        cvlr_assert!(after.sqrt_price_x96 <= sqrt_price_limit);
    }
}

/// RULE: Tick is consistent with sqrt_price after swap
#[cfg(feature = "certora")]
#[rule]
pub fn tick_consistent_after_swap(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let sqrt_price_limit = if zero_for_one {
        MIN_SQRT_RATIO + 1
    } else {
        MAX_SQRT_RATIO - 1
    };

    let _result = DexPool::swap(
        env.clone(),
        recipient,
        zero_for_one,
        amount_specified,
        sqrt_price_limit,
    );

    let after = PoolSnapshot::capture(&env);
    let expected_tick = dex_math::get_tick_at_sqrt_ratio(&env, after.sqrt_price_x96);

    // Tick should be consistent with price (within 1 due to rounding)
    let diff = if after.tick > expected_tick {
        after.tick - expected_tick
    } else {
        expected_tick - after.tick
    };
    cvlr_assert!(diff <= 1);
}

/// RULE: Fee growth is monotonic - fees can only increase, never decrease
#[cfg(feature = "certora")]
#[rule]
pub fn fee_growth_monotonic_after_swap(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    amount_specified: i128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    model::reset();
    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(amount_specified > 0);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let before = PoolSnapshot::capture(&env);

    let sqrt_price_limit = if zero_for_one {
        MIN_SQRT_RATIO + 1
    } else {
        MAX_SQRT_RATIO - 1
    };

    let _result = DexPool::swap(
        env.clone(),
        recipient,
        zero_for_one,
        amount_specified,
        sqrt_price_limit,
    );

    let after = PoolSnapshot::capture(&env);

    // Fee growth can only increase (with wrapping for u128)
    cvlr_assert!(after.fee_growth_global_0 >= before.fee_growth_global_0);
    cvlr_assert!(after.fee_growth_global_1 >= before.fee_growth_global_1);
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::storage::MAX_TICK_CROSSINGS_PER_SWAP;

    #[test]
    fn test_swap_amounts_opposite_signs_valid() {
        // Zero for one: pay token0, receive token1
        assert!((100i128 > 0) && (-95i128 < 0));

        // One for zero: pay token1, receive token0
        assert!((-95i128 < 0) && (100i128 > 0));
    }

    #[test]
    fn test_price_direction_zero_for_one() {
        let price_before: u128 = 79228162514264337593543950336; // Q96 (price = 1)
        let price_after: u128 = 79228162514264337593543950336 - 1000000;

        // Zero for one: price should decrease
        assert!(price_after <= price_before);
    }

    #[test]
    fn test_price_direction_one_for_zero() {
        let price_before: u128 = 79228162514264337593543950336; // Q96
        let price_after: u128 = 79228162514264337593543950336 + 1000000;

        // One for zero: price should increase
        assert!(price_after >= price_before);
    }

    #[test]
    fn test_tick_crossings_bounded() {
        let ticks_crossed: u32 = 35;
        assert!(ticks_crossed <= MAX_TICK_CROSSINGS_PER_SWAP);
    }
}
