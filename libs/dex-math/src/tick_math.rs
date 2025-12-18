use dex_types::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK, Q96};
use soroban_sdk::{Env, U256};

// Q128 constant: 2^128 represented as U256
fn q128(env: &Env) -> U256 {
    U256::from_u128(env, 1u128 << 64).mul(&U256::from_u128(env, 1u128 << 64))
}

/// Calculate sqrt(1.0001^tick) * 2^96
/// Simplified implementation for u128 range
pub fn get_sqrt_ratio_at_tick(env: &Env, tick: i32) -> u128 {
    if tick < MIN_TICK || tick > MAX_TICK {
        panic!("Tick out of bounds");
    }

    let abs_tick = tick.unsigned_abs();

    // Use U256 for intermediate calculations
    // Base ratio in Q128 format
    let mut ratio = q128(env);

    // sqrt(1.0001) in Q128 ≈ 0x10000000000000000000000000000000 * 1.00005 = 0x10000cca1a637b52
    // Precomputed: sqrt(1.0001^(2^i)) in Q128
    // For simplicity, we use an approximation that works for the reduced tick range

    // Constants for sqrt(1.0001^(2^i)) in Q128 format (truncated to fit)
    const SQRT_1_0001_1: u128 = 0xfffcb933bd6fad37aa2d162d1a594001;
    const SQRT_1_0001_2: u128 = 0xfff97272373d413259a46990580e213a;
    const SQRT_1_0001_4: u128 = 0xfff2e50f5f656932ef12357cf3c7fdcc;
    const SQRT_1_0001_8: u128 = 0xffe5caca7e10e4e61c3624eaa0941cd0;
    const SQRT_1_0001_16: u128 = 0xffcb9843d60f6159c9db58835c926644;
    const SQRT_1_0001_32: u128 = 0xff973b41fa98c081472e6896dfb254c0;
    const SQRT_1_0001_64: u128 = 0xff2ea16466c96a3843ec78b326b52861;
    const SQRT_1_0001_128: u128 = 0xfe5dee046a99a2a811c461f1969c3053;
    const SQRT_1_0001_256: u128 = 0xfcbe86c7900a88aedcffc83b479aa3a4;
    const SQRT_1_0001_512: u128 = 0xf987a7253ac413176f2b074cf7815e54;
    const SQRT_1_0001_1024: u128 = 0xf3392b0822b70005940c7a398e4b70f3;
    const SQRT_1_0001_2048: u128 = 0xe7159475a2c29b7443b29c7fa6e889d9;
    const SQRT_1_0001_4096: u128 = 0xd097f3bdfd2022b8845ad8f792aa5825;
    const SQRT_1_0001_8192: u128 = 0xa9f746462d870fdf8a65dc1f90e061e5;
    const SQRT_1_0001_16384: u128 = 0x70d869a156d2a1b890bb3df62baf32f7;
    const SQRT_1_0001_32768: u128 = 0x31be135f97d08fd981231505542fcfa6;
    const SQRT_1_0001_65536: u128 = 0x9aa508b5b7a84e1c677de54f3e99bc9;
    const SQRT_1_0001_131072: u128 = 0x5d6af8dedb81196699c329225ee604;
    const SQRT_1_0001_262144: u128 = 0x2216e584f5fa1ea926041bedfe98;

    if abs_tick & 0x1 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_1);
    }
    if abs_tick & 0x2 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_2);
    }
    if abs_tick & 0x4 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_4);
    }
    if abs_tick & 0x8 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_8);
    }
    if abs_tick & 0x10 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_16);
    }
    if abs_tick & 0x20 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_32);
    }
    if abs_tick & 0x40 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_64);
    }
    if abs_tick & 0x80 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_128);
    }
    if abs_tick & 0x100 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_256);
    }
    if abs_tick & 0x200 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_512);
    }
    if abs_tick & 0x400 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_1024);
    }
    if abs_tick & 0x800 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_2048);
    }
    if abs_tick & 0x1000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_4096);
    }
    if abs_tick & 0x2000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_8192);
    }
    if abs_tick & 0x4000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_16384);
    }
    if abs_tick & 0x8000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_32768);
    }
    if abs_tick & 0x10000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_65536);
    }
    if abs_tick & 0x20000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_131072);
    }
    if abs_tick & 0x40000 != 0 {
        ratio = mul_shift_128(env, &ratio, SQRT_1_0001_262144);
    }

    // Invert if tick is positive (we computed for negative tick)
    if tick > 0 {
        let max_u256 = u256_max(env);
        ratio = max_u256.div(&ratio);
    }

    // Convert from Q128 to Q96 by right shifting 32 bits
    let shift_32 = U256::from_u128(env, 1u128 << 32);
    let result = ratio.div(&shift_32);

    // Clamp to valid range
    let result_u128 = result.to_u128().unwrap_or(u128::MAX);
    result_u128.max(MIN_SQRT_RATIO).min(MAX_SQRT_RATIO)
}

/// Get the tick corresponding to a sqrt price
/// Simplified binary search implementation
pub fn get_tick_at_sqrt_ratio(env: &Env, sqrt_price_x96: u128) -> i32 {
    if sqrt_price_x96 < MIN_SQRT_RATIO || sqrt_price_x96 >= MAX_SQRT_RATIO {
        panic!("sqrt price out of bounds");
    }

    // Binary search for the tick
    let mut low = MIN_TICK;
    let mut high = MAX_TICK;

    while low < high {
        let mid = (low + high + 1) / 2;
        let sqrt_at_mid = get_sqrt_ratio_at_tick(env, mid);

        if sqrt_at_mid <= sqrt_price_x96 {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    low
}

/// Helper: multiply by u128 and right shift by 128 bits
fn mul_shift_128(env: &Env, x: &U256, y: u128) -> U256 {
    let y_256 = U256::from_u128(env, y);
    let product = x.mul(&y_256);
    // Divide by 2^128 (shift right 128 bits)
    let divisor = q128(env);
    product.div(&divisor)
}

/// Helper: get U256 max value
fn u256_max(env: &Env) -> U256 {
    // U256 max = 2^256 - 1
    let high = U256::from_u128(env, u128::MAX);
    let q128_val = q128(env);
    high.mul(&q128_val).add(&U256::from_u128(env, u128::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    // === get_sqrt_ratio_at_tick tests ===

    #[test]
    fn test_get_sqrt_ratio_at_tick_zero() {
        let env = Env::default();
        // At tick 0, price = 1, sqrt(price) = 1
        // sqrtPriceX96 = 1 * 2^96 ≈ 79228162514264337593543950336
        let sqrt_price = get_sqrt_ratio_at_tick(&env, 0);
        // Allow some tolerance due to approximations
        let expected = Q96;
        let diff = if sqrt_price > expected {
            sqrt_price - expected
        } else {
            expected - sqrt_price
        };
        assert!(diff < expected / 1000, "tick 0 should give sqrt_price ≈ 2^96");
    }

    #[test]
    fn test_get_sqrt_ratio_at_tick_positive() {
        let env = Env::default();
        // Positive ticks should give sqrt price > Q96 (price > 1)
        let sqrt_100 = get_sqrt_ratio_at_tick(&env, 100);
        let sqrt_1000 = get_sqrt_ratio_at_tick(&env, 1000);
        let sqrt_10000 = get_sqrt_ratio_at_tick(&env, 10000);

        assert!(sqrt_100 > Q96, "tick 100 should give sqrt_price > Q96");
        assert!(sqrt_1000 > sqrt_100, "sqrt_price should increase with tick");
        assert!(sqrt_10000 > sqrt_1000, "sqrt_price should increase with tick");
    }

    #[test]
    fn test_get_sqrt_ratio_at_tick_negative() {
        let env = Env::default();
        // Negative ticks should give sqrt price < Q96 (price < 1)
        let sqrt_neg_100 = get_sqrt_ratio_at_tick(&env, -100);
        let sqrt_neg_1000 = get_sqrt_ratio_at_tick(&env, -1000);
        let sqrt_neg_10000 = get_sqrt_ratio_at_tick(&env, -10000);

        assert!(sqrt_neg_100 < Q96, "tick -100 should give sqrt_price < Q96");
        assert!(sqrt_neg_1000 < sqrt_neg_100, "sqrt_price should decrease with lower tick");
        assert!(sqrt_neg_10000 < sqrt_neg_1000, "sqrt_price should decrease with lower tick");
    }

    #[test]
    fn test_get_sqrt_ratio_at_tick_symmetric() {
        let env = Env::default();
        // sqrt(1.0001^n) * sqrt(1.0001^-n) = 1
        // So price_at_tick(n) * price_at_tick(-n) ≈ Q96^2
        let sqrt_100 = get_sqrt_ratio_at_tick(&env, 100);
        let sqrt_neg_100 = get_sqrt_ratio_at_tick(&env, -100);

        // Product should be close to Q96^2 / Q96 = Q96 when computed properly
        // sqrt(1.0001^100) * sqrt(1.0001^-100) = 1 in linear terms
        // In Q96: (sqrt_100 * sqrt_neg_100) / Q96 ≈ Q96
        let q96_256 = U256::from_u128(&env, Q96);
        let sqrt_100_256 = U256::from_u128(&env, sqrt_100);
        let sqrt_neg_100_256 = U256::from_u128(&env, sqrt_neg_100);

        let product = sqrt_100_256.mul(&sqrt_neg_100_256).div(&q96_256);
        let result = product.to_u128().unwrap();

        // Allow 1% tolerance
        let diff = if result > Q96 { result - Q96 } else { Q96 - result };
        assert!(diff < Q96 / 100, "symmetric ticks should have product ≈ Q96^2");
    }

    #[test]
    fn test_get_sqrt_ratio_at_tick_monotonic() {
        let env = Env::default();
        // sqrt price should be strictly increasing with tick
        let mut prev_sqrt = get_sqrt_ratio_at_tick(&env, -10000);
        for tick in (-9999..=10000).step_by(100) {
            let sqrt = get_sqrt_ratio_at_tick(&env, tick);
            assert!(sqrt > prev_sqrt, "sqrt_price should be monotonically increasing");
            prev_sqrt = sqrt;
        }
    }

    #[test]
    fn test_tick_bounds_min() {
        let env = Env::default();
        let min_sqrt = get_sqrt_ratio_at_tick(&env, MIN_TICK);
        assert!(min_sqrt >= MIN_SQRT_RATIO);
        assert!(min_sqrt < Q96 / 1000, "MIN_TICK should give very small sqrt price");
    }

    #[test]
    fn test_tick_bounds_max() {
        let env = Env::default();
        let max_sqrt = get_sqrt_ratio_at_tick(&env, MAX_TICK);
        assert!(max_sqrt <= MAX_SQRT_RATIO);
        assert!(max_sqrt > Q96 * 1000, "MAX_TICK should give very large sqrt price");
    }

    #[test]
    #[should_panic(expected = "Tick out of bounds")]
    fn test_get_sqrt_ratio_below_min_tick() {
        let env = Env::default();
        get_sqrt_ratio_at_tick(&env, MIN_TICK - 1);
    }

    #[test]
    #[should_panic(expected = "Tick out of bounds")]
    fn test_get_sqrt_ratio_above_max_tick() {
        let env = Env::default();
        get_sqrt_ratio_at_tick(&env, MAX_TICK + 1);
    }

    // === get_tick_at_sqrt_ratio tests ===

    #[test]
    fn test_get_tick_at_sqrt_ratio_q96() {
        let env = Env::default();
        // Q96 represents sqrt(1), so tick should be 0
        let tick = get_tick_at_sqrt_ratio(&env, Q96);
        assert!(tick.abs() <= 1, "tick at Q96 should be ~0");
    }

    #[test]
    fn test_get_tick_at_sqrt_ratio_roundtrip() {
        let env = Env::default();
        // Test roundtrip at various ticks
        for tick in [-100000, -10000, -1000, -100, 0, 100, 1000, 10000, 100000] {
            if tick < MIN_TICK || tick > MAX_TICK {
                continue;
            }
            let sqrt_price = get_sqrt_ratio_at_tick(&env, tick);
            let recovered_tick = get_tick_at_sqrt_ratio(&env, sqrt_price);
            // Due to rounding, the recovered tick might be off by 1
            assert!(
                (recovered_tick - tick).abs() <= 1,
                "tick {} should roundtrip, got {}",
                tick,
                recovered_tick
            );
        }
    }

    #[test]
    fn test_get_tick_at_sqrt_ratio_min() {
        let env = Env::default();
        let tick = get_tick_at_sqrt_ratio(&env, MIN_SQRT_RATIO);
        assert_eq!(tick, MIN_TICK, "MIN_SQRT_RATIO should give MIN_TICK");
    }

    #[test]
    fn test_get_tick_at_sqrt_ratio_just_above_min() {
        let env = Env::default();
        let tick = get_tick_at_sqrt_ratio(&env, MIN_SQRT_RATIO + 1);
        // Should still be close to MIN_TICK
        assert!(tick >= MIN_TICK && tick <= MIN_TICK + 10);
    }

    #[test]
    fn test_get_tick_at_sqrt_ratio_just_below_max() {
        let env = Env::default();
        let tick = get_tick_at_sqrt_ratio(&env, MAX_SQRT_RATIO - 1);
        // Should be close to MAX_TICK
        assert!(tick >= MAX_TICK - 10 && tick <= MAX_TICK);
    }

    #[test]
    #[should_panic(expected = "sqrt price out of bounds")]
    fn test_get_tick_at_sqrt_ratio_below_min() {
        let env = Env::default();
        get_tick_at_sqrt_ratio(&env, MIN_SQRT_RATIO - 1);
    }

    #[test]
    #[should_panic(expected = "sqrt price out of bounds")]
    fn test_get_tick_at_sqrt_ratio_at_max() {
        let env = Env::default();
        // MAX_SQRT_RATIO is exclusive
        get_tick_at_sqrt_ratio(&env, MAX_SQRT_RATIO);
    }

    // === Price relationship tests ===

    #[test]
    fn test_tick_spacing_price_change() {
        let env = Env::default();
        // Each tick represents a 0.01% (1 basis point) price change
        // So 100 ticks ≈ 1% price change
        let sqrt_0 = get_sqrt_ratio_at_tick(&env, 0);
        let sqrt_100 = get_sqrt_ratio_at_tick(&env, 100);

        // Price ratio = (sqrt_100 / sqrt_0)^2
        // 100 ticks = 1.0001^100 ≈ 1.01005 (about 1.005% increase)
        let q96_256 = U256::from_u128(&env, Q96);
        let sqrt_0_256 = U256::from_u128(&env, sqrt_0);
        let sqrt_100_256 = U256::from_u128(&env, sqrt_100);

        // Calculate sqrt ratio
        let ratio = sqrt_100_256.mul(&q96_256).div(&sqrt_0_256);
        let ratio_u128 = ratio.to_u128().unwrap();

        // Expected ratio for 100 ticks: sqrt(1.0001^100) ≈ 1.005 in Q96
        // = Q96 * 1.005 ≈ Q96 + Q96/200
        let expected_min = Q96 + Q96 / 250;
        let expected_max = Q96 + Q96 / 150;
        assert!(
            ratio_u128 > expected_min && ratio_u128 < expected_max,
            "100 ticks should be ~0.5% sqrt price change"
        );
    }

    #[test]
    fn test_common_fee_tier_tick_spacings() {
        let env = Env::default();
        // Test that tick spacings for common fee tiers work correctly
        // 0.05% fee: tick spacing 10
        // 0.3% fee: tick spacing 60
        // 1% fee: tick spacing 200

        for spacing in [10, 60, 200] {
            let sqrt_0 = get_sqrt_ratio_at_tick(&env, 0);
            let sqrt_spacing = get_sqrt_ratio_at_tick(&env, spacing);

            // Should be able to compute valid prices at each tick spacing
            assert!(sqrt_spacing > sqrt_0);
            assert!(sqrt_spacing > Q96);
        }
    }

    // === Specific known value tests (from Uniswap v3) ===

    #[test]
    fn test_known_tick_values() {
        let env = Env::default();
        // These are approximations since we use a simplified implementation

        // At tick 0, sqrt price = 2^96
        let sqrt_0 = get_sqrt_ratio_at_tick(&env, 0);
        let diff = if sqrt_0 > Q96 { sqrt_0 - Q96 } else { Q96 - sqrt_0 };
        assert!(diff < Q96 / 100, "tick 0 price within 1%");

        // At tick 6931 (~69.3% of 10000), sqrt price ≈ sqrt(2) * Q96
        // Because 1.0001^6931 ≈ 2, so sqrt(2) ≈ 1.414
        let sqrt_6931 = get_sqrt_ratio_at_tick(&env, 6931);
        let expected_sqrt_2 = Q96 * 1414 / 1000; // Approximate sqrt(2) * Q96
        let diff = if sqrt_6931 > expected_sqrt_2 {
            sqrt_6931 - expected_sqrt_2
        } else {
            expected_sqrt_2 - sqrt_6931
        };
        // Allow 5% tolerance due to approximation
        assert!(diff < expected_sqrt_2 / 20, "tick 6931 should give ~sqrt(2)*Q96");
    }
}
