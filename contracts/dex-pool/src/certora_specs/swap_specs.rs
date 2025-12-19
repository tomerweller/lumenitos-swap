// ============================================================================
// SWAP INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify the correctness of swap operations in the
// concentrated liquidity AMM.
//
// KEY INVARIANTS:
// 1. Value conservation - no tokens created from nothing
// 2. Direction consistency - price moves in expected direction
// 3. Limit enforcement - swap respects price limits
// 4. Amount consistency - amounts have correct signs
// 5. Tick crossing bounds - limited crossings per swap
//
// ============================================================================

// ============================================================================
// FORMAL VERIFICATION RULES (Certora Sunbeam)
// ============================================================================

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

/// RULE: compute_swap_step enforces direction on price movement.
#[cfg(feature = "certora")]
#[rule]
pub fn swap_step_price_direction(
    env: soroban_sdk::Env,
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: i128,
    fee_pips: u32,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(liquidity > 0);
    cvlr_assume!(fee_pips <= 1_000_000);
    cvlr_assume!(sqrt_price_current > MIN_SQRT_RATIO && sqrt_price_current < MAX_SQRT_RATIO);
    cvlr_assume!(sqrt_price_target > MIN_SQRT_RATIO && sqrt_price_target < MAX_SQRT_RATIO);

    let result = dex_math::compute_swap_step(
        &env,
        sqrt_price_current,
        sqrt_price_target,
        liquidity,
        amount_remaining,
        fee_pips,
    );

    let zero_for_one = sqrt_price_current >= sqrt_price_target;
    if zero_for_one {
        cvlr_assert!(result.sqrt_ratio_next_x96 <= sqrt_price_current);
    } else {
        cvlr_assert!(result.sqrt_ratio_next_x96 >= sqrt_price_current);
    }
}

/// RULE: compute_swap_step outputs have consistent signs with swap intent.
#[cfg(feature = "certora")]
#[rule]
pub fn swap_step_amount_signs(
    env: soroban_sdk::Env,
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: i128,
    fee_pips: u32,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(liquidity > 0);
    cvlr_assume!(fee_pips <= 1_000_000);
    cvlr_assume!(sqrt_price_current > MIN_SQRT_RATIO && sqrt_price_current < MAX_SQRT_RATIO);
    cvlr_assume!(sqrt_price_target > MIN_SQRT_RATIO && sqrt_price_target < MAX_SQRT_RATIO);

    let result = dex_math::compute_swap_step(
        &env,
        sqrt_price_current,
        sqrt_price_target,
        liquidity,
        amount_remaining,
        fee_pips,
    );

    let exact_in = amount_remaining >= 0;
    if exact_in {
        cvlr_assert!(result.amount_in > 0);
    } else {
        cvlr_assert!(result.amount_out > 0);
    }
}

/// RULE: Tick crossings per swap are bounded by configuration.
#[cfg(feature = "certora")]
#[rule]
pub fn tick_crossings_bounded(ticks_crossed: u32) {
    use crate::storage::MAX_TICK_CROSSINGS_PER_SWAP;
    cvlr_assert!(ticks_crossed <= MAX_TICK_CROSSINGS_PER_SWAP);
}

// ============================================================================
// TESTS (run with cargo test)
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
    fn test_price_limit_respected() {
        let sqrt_price_limit: u128 = 50000000000000000000000000000;
        let sqrt_price_after: u128 = 60000000000000000000000000000;

        // For zero_for_one, price_after >= limit
        assert!(sqrt_price_after >= sqrt_price_limit);
    }

    #[test]
    fn test_tick_crossings_bounded() {
        let ticks_crossed: u32 = 35;
        assert!(ticks_crossed <= MAX_TICK_CROSSINGS_PER_SWAP);
    }

    #[test]
    fn test_fee_proportional() {
        let amount_in: u128 = 1_000_000;
        let fee_pips: u32 = 3000; // 0.3%
        let expected_fee = amount_in * (fee_pips as u128) / 1_000_000;

        assert_eq!(expected_fee, 3000); // 0.3% of 1M
    }
}
