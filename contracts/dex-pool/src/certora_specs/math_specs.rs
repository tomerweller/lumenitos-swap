// ============================================================================
// MATH FUNCTION SPECIFICATIONS
// ============================================================================
//
// These specifications verify the correctness of pure mathematical functions
// in dex_math. Since these are pure functions (no state), we can verify
// their properties directly.
//
// ============================================================================

#[cfg(feature = "certora")]
use soroban_sdk::Env;

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume};

/// RULE: Sqrt ratio is strictly monotonically increasing with tick
/// This is a fundamental property: higher tick = higher price
#[cfg(feature = "certora")]
#[rule]
pub fn sqrt_ratio_strictly_monotonic(env: Env, tick1: i32, tick2: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    // Constrain to valid tick range
    cvlr_assume!(tick1 >= MIN_TICK && tick1 <= MAX_TICK);
    cvlr_assume!(tick2 >= MIN_TICK && tick2 <= MAX_TICK);
    cvlr_assume!(tick1 < tick2);

    let ratio1 = dex_math::get_sqrt_ratio_at_tick(&env, tick1);
    let ratio2 = dex_math::get_sqrt_ratio_at_tick(&env, tick2);

    // Strict monotonicity: tick1 < tick2 => ratio1 < ratio2
    cvlr_assert!(ratio1 < ratio2);
}

/// RULE: Tick-to-ratio-to-tick roundtrip is consistent
/// get_tick_at_sqrt_ratio(get_sqrt_ratio_at_tick(t)) should return t (or t±1 due to rounding)
#[cfg(feature = "certora")]
#[rule]
pub fn tick_ratio_roundtrip(env: Env, tick: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);

    let sqrt_ratio = dex_math::get_sqrt_ratio_at_tick(&env, tick);
    let recovered_tick = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_ratio);

    // Due to discrete tick spacing, recovered tick may differ by at most 1
    let diff = if recovered_tick > tick {
        recovered_tick - tick
    } else {
        tick - recovered_tick
    };

    cvlr_assert!(diff <= 1);
}

/// RULE: mul_div_rounding_up always >= mul_div
/// This ensures rounding up actually rounds up
#[cfg(feature = "certora")]
#[rule]
pub fn mul_div_rounding_relationship(env: Env, a: u128, b: u128, c: u128) {
    // Avoid division by zero and overflow
    cvlr_assume!(c > 0);
    cvlr_assume!(a > 0 && b > 0);

    let result_down = dex_math::mul_div(&env, a, b, c);
    let result_up = dex_math::mul_div_rounding_up(&env, a, b, c);

    // Rounding up should always be >= rounding down
    cvlr_assert!(result_up >= result_down);

    // And they should differ by at most 1
    cvlr_assert!(result_up - result_down <= 1);
}

/// RULE: add_delta correctness for positive delta
#[cfg(feature = "certora")]
#[rule]
pub fn add_delta_positive_correct(liquidity: u128, delta: i128) {
    cvlr_assume!(delta > 0);
    cvlr_assume!(liquidity <= u128::MAX - (delta as u128)); // No overflow

    let result = dex_math::add_delta(liquidity, delta);

    // Result should equal liquidity + delta
    cvlr_assert!(result == liquidity + (delta as u128));
}

/// RULE: add_delta correctness for negative delta
#[cfg(feature = "certora")]
#[rule]
pub fn add_delta_negative_correct(liquidity: u128, delta: i128) {
    cvlr_assume!(delta < 0);
    let abs_delta = (-delta) as u128;
    cvlr_assume!(liquidity >= abs_delta); // No underflow

    let result = dex_math::add_delta(liquidity, delta);

    // Result should equal liquidity - |delta|
    cvlr_assert!(result == liquidity - abs_delta);
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use soroban_sdk::Env;
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    #[test]
    fn test_sqrt_ratio_monotonic() {
        let env = Env::default();
        let r1 = dex_math::get_sqrt_ratio_at_tick(&env, -100);
        let r2 = dex_math::get_sqrt_ratio_at_tick(&env, 0);
        let r3 = dex_math::get_sqrt_ratio_at_tick(&env, 100);

        assert!(r1 < r2);
        assert!(r2 < r3);
    }

    #[test]
    fn test_sqrt_ratio_bounds() {
        let env = Env::default();

        let r_min = dex_math::get_sqrt_ratio_at_tick(&env, MIN_TICK);
        let r_max = dex_math::get_sqrt_ratio_at_tick(&env, MAX_TICK);

        assert!(r_min >= MIN_SQRT_RATIO);
        assert!(r_max <= MAX_SQRT_RATIO);
    }

    #[test]
    fn test_tick_roundtrip() {
        let env = Env::default();

        for tick in [-1000, -100, 0, 100, 1000] {
            let sqrt_ratio = dex_math::get_sqrt_ratio_at_tick(&env, tick);
            let recovered = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_ratio);
            let diff = (recovered - tick).abs();
            assert!(diff <= 1, "Tick {} roundtrip diff {}", tick, diff);
        }
    }

    #[test]
    fn test_mul_div_rounding() {
        let env = Env::default();

        let a: u128 = 100;
        let b: u128 = 3;
        let c: u128 = 10;

        let down = dex_math::mul_div(&env, a, b, c);
        let up = dex_math::mul_div_rounding_up(&env, a, b, c);

        assert!(up >= down);
        assert!(up - down <= 1);
    }

    #[test]
    fn test_add_delta() {
        // Adding liquidity
        let result = dex_math::add_delta(1000, 500);
        assert_eq!(result, 1500);

        // Removing liquidity
        let result = dex_math::add_delta(1000, -300);
        assert_eq!(result, 700);
    }
}
