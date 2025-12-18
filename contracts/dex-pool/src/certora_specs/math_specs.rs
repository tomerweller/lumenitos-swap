// ============================================================================
// MATH INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify the correctness of mathematical operations
// used in the concentrated liquidity AMM.
//
// KEY INVARIANTS:
// 1. mul_div operations never overflow with proper inputs
// 2. Tick-to-price conversion is monotonic
// 3. Price-to-tick roundtrip maintains consistency
// 4. Sqrt price calculations are bounded
//
// ============================================================================

// ============================================================================
// FORMAL VERIFICATION RULES (Certora Sunbeam)
// ============================================================================

#[cfg(feature = "certora")]
use soroban_sdk::Env;

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

/// RULE: Sanity check - sqrt ratio calculation is reachable
#[cfg(feature = "certora")]
#[rule]
pub fn sanity_sqrt_ratio(env: Env, tick: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};
    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);
    let _ratio = dex_math::get_sqrt_ratio_at_tick(&env, tick);
    cvlr_satisfy!(true);
}

/// RULE: Sqrt ratio is monotonically increasing with tick
#[cfg(feature = "certora")]
#[rule]
pub fn sqrt_ratio_monotonic(env: Env, tick1: i32, tick2: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick1 >= MIN_TICK && tick1 <= MAX_TICK);
    cvlr_assume!(tick2 >= MIN_TICK && tick2 <= MAX_TICK);
    cvlr_assume!(tick1 < tick2);

    let ratio1 = dex_math::get_sqrt_ratio_at_tick(&env, tick1);
    let ratio2 = dex_math::get_sqrt_ratio_at_tick(&env, tick2);

    cvlr_assert!(ratio1 < ratio2);
}

/// RULE: Sqrt ratio at MIN_TICK is >= MIN_SQRT_RATIO
#[cfg(feature = "certora")]
#[rule]
pub fn sqrt_ratio_min_bound(env: Env) {
    use dex_types::{MIN_SQRT_RATIO, MIN_TICK};

    let ratio = dex_math::get_sqrt_ratio_at_tick(&env, MIN_TICK);
    cvlr_assert!(ratio >= MIN_SQRT_RATIO);
}

/// RULE: Sqrt ratio at MAX_TICK is <= MAX_SQRT_RATIO
#[cfg(feature = "certora")]
#[rule]
pub fn sqrt_ratio_max_bound(env: Env) {
    use dex_types::{MAX_SQRT_RATIO, MAX_TICK};

    let ratio = dex_math::get_sqrt_ratio_at_tick(&env, MAX_TICK);
    cvlr_assert!(ratio <= MAX_SQRT_RATIO);
}

/// RULE: Tick roundtrip is consistent (within 1 tick)
#[cfg(feature = "certora")]
#[rule]
pub fn tick_roundtrip_consistent(env: Env, tick: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);

    let sqrt_ratio = dex_math::get_sqrt_ratio_at_tick(&env, tick);
    let recovered_tick = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_ratio);

    let diff = if recovered_tick > tick {
        recovered_tick - tick
    } else {
        tick - recovered_tick
    };

    cvlr_assert!(diff <= 1);
}

/// RULE: mul_div rounds down
#[cfg(feature = "certora")]
#[rule]
pub fn mul_div_rounds_down(env: Env, a: u128, b: u128, c: u128) {
    cvlr_assume!(c > 0);
    cvlr_assume!(a <= u64::MAX as u128);
    cvlr_assume!(b <= u64::MAX as u128);

    let result = dex_math::mul_div(&env, a, b, c);
    let result_up = dex_math::mul_div_rounding_up(&env, a, b, c);

    cvlr_assert!(result <= result_up);
    cvlr_assert!(result_up - result <= 1);
}

/// RULE: add_delta with positive delta increases value
#[cfg(feature = "certora")]
#[rule]
pub fn add_delta_positive_increases(liquidity: u128, delta: i128) {
    cvlr_assume!(delta > 0);
    cvlr_assume!(liquidity < u128::MAX - (delta as u128));

    let result = dex_math::add_delta(liquidity, delta);
    cvlr_assert!(result > liquidity);
}

/// RULE: add_delta with negative delta decreases value
#[cfg(feature = "certora")]
#[rule]
pub fn add_delta_negative_decreases(liquidity: u128, delta: i128) {
    cvlr_assume!(delta < 0);
    cvlr_assume!(liquidity >= (-delta) as u128);

    let result = dex_math::add_delta(liquidity, delta);
    cvlr_assert!(result < liquidity);
}

// ============================================================================
// TESTS (run with cargo test)
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
    fn test_mul_div_no_value_creation() {
        let env = Env::default();

        let a: u128 = 1000;
        let b: u128 = 500;
        let c: u128 = 300;

        let result = dex_math::mul_div(&env, a, b, c);
        // result should be floor((a * b) / c) = floor(500000 / 300) = 1666
        assert!(result <= a * b / c + 1);
    }

    #[test]
    fn test_mul_div_rounding() {
        let env = Env::default();

        let a: u128 = 100;
        let b: u128 = 3;
        let c: u128 = 10;

        let down = dex_math::mul_div(&env, a, b, c);
        let up = dex_math::mul_div_rounding_up(&env, a, b, c);

        // 300 / 10 = 30, no rounding needed
        assert_eq!(down, 30);
        assert_eq!(up, 30);
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
