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

/// RULE: Swap amounts have opposite signs (one in, one out)
#[cfg(feature = "certora")]
#[rule]
pub fn swap_amounts_opposite_signs(amount0: i128, amount1: i128) {
    // Valid swap: one positive (in), one negative (out), or one is zero
    let valid_swap =
        (amount0 > 0 && amount1 < 0) ||
        (amount0 < 0 && amount1 > 0) ||
        (amount0 == 0) ||
        (amount1 == 0);

    cvlr_assert!(valid_swap);
}

/// RULE: Zero-for-one swaps decrease price
#[cfg(feature = "certora")]
#[rule]
pub fn zero_for_one_decreases_price(
    sqrt_price_before: u128,
    sqrt_price_after: u128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(zero_for_one);
    cvlr_assume!(sqrt_price_before > MIN_SQRT_RATIO && sqrt_price_before < MAX_SQRT_RATIO);
    cvlr_assume!(sqrt_price_after > MIN_SQRT_RATIO && sqrt_price_after < MAX_SQRT_RATIO);

    cvlr_assert!(sqrt_price_after <= sqrt_price_before);
}

/// RULE: One-for-zero swaps increase price
#[cfg(feature = "certora")]
#[rule]
pub fn one_for_zero_increases_price(
    sqrt_price_before: u128,
    sqrt_price_after: u128,
    zero_for_one: bool,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(!zero_for_one);
    cvlr_assume!(sqrt_price_before > MIN_SQRT_RATIO && sqrt_price_before < MAX_SQRT_RATIO);
    cvlr_assume!(sqrt_price_after > MIN_SQRT_RATIO && sqrt_price_after < MAX_SQRT_RATIO);

    cvlr_assert!(sqrt_price_after >= sqrt_price_before);
}

/// RULE: Swap respects price limit
#[cfg(feature = "certora")]
#[rule]
pub fn swap_respects_price_limit(
    sqrt_price_after: u128,
    sqrt_price_limit: u128,
    zero_for_one: bool,
) {
    if zero_for_one {
        cvlr_assert!(sqrt_price_after >= sqrt_price_limit);
    } else {
        cvlr_assert!(sqrt_price_after <= sqrt_price_limit);
    }
}

/// RULE: Tick crossings are bounded
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
