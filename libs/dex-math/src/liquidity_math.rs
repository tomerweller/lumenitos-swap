use crate::full_math::mul_div;
use dex_types::Q96;
use soroban_sdk::Env;

/// Calculate liquidity from token amounts for a price range
pub fn get_liquidity_for_amounts(
    env: &Env,
    sqrt_ratio_x96: u128,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    amount0: u128,
    amount1: u128,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    if sqrt_ratio_x96 <= sqrt_ratio_lower {
        // Current price below range - all token0
        get_liquidity_for_amount0(env, sqrt_ratio_lower, sqrt_ratio_upper, amount0)
    } else if sqrt_ratio_x96 < sqrt_ratio_upper {
        // Current price in range - both tokens
        let liquidity0 =
            get_liquidity_for_amount0(env, sqrt_ratio_x96, sqrt_ratio_upper, amount0);
        let liquidity1 =
            get_liquidity_for_amount1(env, sqrt_ratio_lower, sqrt_ratio_x96, amount1);
        liquidity0.min(liquidity1)
    } else {
        // Current price above range - all token1
        get_liquidity_for_amount1(env, sqrt_ratio_lower, sqrt_ratio_upper, amount1)
    }
}

/// Calculate liquidity from amount0
/// L = amount0 * sqrt_pa * sqrt_pb / (sqrt_pb - sqrt_pa)
fn get_liquidity_for_amount0(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    amount0: u128,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    let intermediate = mul_div(env, sqrt_ratio_lower, sqrt_ratio_upper, Q96);
    mul_div(env, amount0, intermediate, sqrt_ratio_upper - sqrt_ratio_lower)
}

/// Calculate liquidity from amount1
/// L = amount1 / (sqrt_pb - sqrt_pa)
fn get_liquidity_for_amount1(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    amount1: u128,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    mul_div(env, amount1, Q96, sqrt_ratio_upper - sqrt_ratio_lower)
}

/// Get amounts from liquidity for a price range
pub fn get_amounts_for_liquidity(
    env: &Env,
    sqrt_ratio_x96: u128,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    liquidity: u128,
) -> (u128, u128) {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    if sqrt_ratio_x96 <= sqrt_ratio_lower {
        // Below range - all token0
        let amount0 = get_amount0_for_liquidity(env, sqrt_ratio_lower, sqrt_ratio_upper, liquidity);
        (amount0, 0)
    } else if sqrt_ratio_x96 < sqrt_ratio_upper {
        // In range - both tokens
        let amount0 = get_amount0_for_liquidity(env, sqrt_ratio_x96, sqrt_ratio_upper, liquidity);
        let amount1 = get_amount1_for_liquidity(env, sqrt_ratio_lower, sqrt_ratio_x96, liquidity);
        (amount0, amount1)
    } else {
        // Above range - all token1
        let amount1 = get_amount1_for_liquidity(env, sqrt_ratio_lower, sqrt_ratio_upper, liquidity);
        (0, amount1)
    }
}

/// Calculate amount0 from liquidity
fn get_amount0_for_liquidity(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    liquidity: u128,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    mul_div(
        env,
        liquidity << 96,
        sqrt_ratio_upper - sqrt_ratio_lower,
        sqrt_ratio_upper,
    ) / sqrt_ratio_lower
}

/// Calculate amount1 from liquidity
fn get_amount1_for_liquidity(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    liquidity: u128,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    mul_div(env, liquidity, sqrt_ratio_upper - sqrt_ratio_lower, Q96)
}

/// Add signed liquidity delta to unsigned liquidity
pub fn add_delta(liquidity: u128, delta: i128) -> u128 {
    if delta < 0 {
        let abs_delta = (-delta) as u128;
        if liquidity < abs_delta {
            panic!("Liquidity underflow");
        }
        liquidity - abs_delta
    } else {
        let result = liquidity + (delta as u128);
        if result < liquidity {
            panic!("Liquidity overflow");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_types::Q96;
    use soroban_sdk::Env;

    // === add_delta tests ===

    #[test]
    fn test_add_delta_positive() {
        let result = add_delta(100, 50);
        assert_eq!(result, 150);
    }

    #[test]
    fn test_add_delta_negative() {
        let result = add_delta(100, -50);
        assert_eq!(result, 50);
    }

    #[test]
    fn test_add_delta_zero() {
        let result = add_delta(100, 0);
        assert_eq!(result, 100);
    }

    #[test]
    fn test_add_delta_to_zero() {
        let result = add_delta(100, -100);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_add_delta_large_positive() {
        let large = 1u128 << 100;
        let result = add_delta(large, 1000);
        assert_eq!(result, large + 1000);
    }

    #[test]
    fn test_add_delta_large_negative() {
        let large = 1u128 << 100;
        let result = add_delta(large, -1000);
        assert_eq!(result, large - 1000);
    }

    #[test]
    fn test_add_delta_max_i128() {
        let result = add_delta(0, i128::MAX);
        assert_eq!(result, i128::MAX as u128);
    }

    #[test]
    #[should_panic(expected = "Liquidity underflow")]
    fn test_add_delta_underflow() {
        add_delta(50, -100);
    }

    #[test]
    #[should_panic(expected = "Liquidity underflow")]
    fn test_add_delta_underflow_max_negative() {
        add_delta(0, -1);
    }

    // === get_liquidity_for_amounts tests ===

    #[test]
    fn test_get_liquidity_for_amounts_in_range() {
        let env = Env::default();
        let sqrt_price = Q96;
        let sqrt_lower = sqrt_price * 9 / 10; // ~0.9
        let sqrt_upper = sqrt_price * 11 / 10; // ~1.1

        let liquidity = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        assert!(liquidity > 0);
    }

    #[test]
    fn test_get_liquidity_for_amounts_below_range() {
        let env = Env::default();
        // Current price below range - only token0 matters
        let sqrt_price = Q96 * 8 / 10; // 0.8
        let sqrt_lower = Q96 * 9 / 10; // 0.9
        let sqrt_upper = Q96 * 11 / 10; // 1.1

        let liquidity = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            0, // token1 doesn't matter
        );
        assert!(liquidity > 0);

        // Adding more token1 shouldn't change liquidity
        let liquidity_with_token1 = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        assert_eq!(liquidity, liquidity_with_token1);
    }

    #[test]
    fn test_get_liquidity_for_amounts_above_range() {
        let env = Env::default();
        // Current price above range - only token1 matters
        let sqrt_price = Q96 * 12 / 10; // 1.2
        let sqrt_lower = Q96 * 9 / 10; // 0.9
        let sqrt_upper = Q96 * 11 / 10; // 1.1

        let liquidity = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            0, // token0 doesn't matter
            1_000_000_000,
        );
        assert!(liquidity > 0);

        // Adding more token0 shouldn't change liquidity
        let liquidity_with_token0 = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        assert_eq!(liquidity, liquidity_with_token0);
    }

    #[test]
    fn test_get_liquidity_for_amounts_at_lower_boundary() {
        let env = Env::default();
        // Current price exactly at lower boundary
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;

        let liquidity = get_liquidity_for_amounts(
            &env,
            sqrt_lower,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        assert!(liquidity > 0);
    }

    #[test]
    fn test_get_liquidity_for_amounts_at_upper_boundary() {
        let env = Env::default();
        // Current price exactly at upper boundary - should use token1 only
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;

        let liquidity = get_liquidity_for_amounts(
            &env,
            sqrt_upper,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        assert!(liquidity > 0);
    }

    #[test]
    fn test_get_liquidity_for_amounts_order_independent() {
        let env = Env::default();
        let sqrt_price = Q96;
        let sqrt_a = Q96 * 9 / 10;
        let sqrt_b = Q96 * 11 / 10;

        let liquidity_ab = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_a,
            sqrt_b,
            1_000_000_000,
            1_000_000_000,
        );
        let liquidity_ba = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_b,
            sqrt_a,
            1_000_000_000,
            1_000_000_000,
        );
        assert_eq!(liquidity_ab, liquidity_ba);
    }

    #[test]
    fn test_get_liquidity_proportional() {
        let env = Env::default();
        let sqrt_price = Q96;
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;

        let liquidity_1x = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            1_000_000_000,
            1_000_000_000,
        );
        let liquidity_2x = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            2_000_000_000,
            2_000_000_000,
        );

        // Should be proportional (allowing for rounding)
        let ratio = (liquidity_2x * 100) / liquidity_1x;
        assert!(ratio >= 195 && ratio <= 205, "Liquidity should be ~2x");
    }

    // === get_amounts_for_liquidity tests ===

    #[test]
    fn test_get_amounts_for_liquidity_in_range() {
        let env = Env::default();
        let sqrt_price = Q96;
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;
        let liquidity = 1_000_000_000_000u128;

        let (amount0, amount1) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, liquidity);

        // Both amounts should be non-zero when in range
        assert!(amount0 > 0, "amount0 should be > 0 in range");
        assert!(amount1 > 0, "amount1 should be > 0 in range");
    }

    #[test]
    fn test_get_amounts_for_liquidity_below_range() {
        let env = Env::default();
        let sqrt_price = Q96 * 8 / 10; // Below range
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;
        let liquidity = 1_000_000_000_000u128;

        let (amount0, amount1) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, liquidity);

        // Only token0 when below range
        assert!(amount0 > 0, "amount0 should be > 0 below range");
        assert_eq!(amount1, 0, "amount1 should be 0 below range");
    }

    #[test]
    fn test_get_amounts_for_liquidity_above_range() {
        let env = Env::default();
        let sqrt_price = Q96 * 12 / 10; // Above range
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;
        let liquidity = 1_000_000_000_000u128;

        let (amount0, amount1) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, liquidity);

        // Only token1 when above range
        assert_eq!(amount0, 0, "amount0 should be 0 above range");
        assert!(amount1 > 0, "amount1 should be > 0 above range");
    }

    #[test]
    fn test_get_amounts_for_liquidity_proportional() {
        let env = Env::default();
        let sqrt_price = Q96;
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;

        let (amount0_1x, amount1_1x) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, 1_000_000_000);
        let (amount0_2x, amount1_2x) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, 2_000_000_000);

        // Amounts should be proportional to liquidity
        assert_eq!(amount0_2x / 2, amount0_1x);
        assert_eq!(amount1_2x / 2, amount1_1x);
    }

    // === Roundtrip tests ===

    #[test]
    fn test_liquidity_amounts_roundtrip() {
        let env = Env::default();
        // Test above-range scenario (only token1) for more predictable roundtrip
        // Token1 math is simpler: amount1 = L * (sqrt_upper - sqrt_lower) / Q96
        // and L = amount1 * Q96 / (sqrt_upper - sqrt_lower)
        let sqrt_price = Q96 * 12 / 10; // 1.2 - above range
        let sqrt_lower = Q96 * 9 / 10; // 0.9
        let sqrt_upper = Q96 * 11 / 10; // 1.1
        let initial_liquidity = 1_000_000_000_000_000u128;

        // Get amounts for liquidity (only token1 since above range)
        let (amount0, amount1) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, initial_liquidity);

        assert_eq!(amount0, 0, "should not have token0 above range");
        assert!(amount1 > 0, "should have token1 above range");

        // Get liquidity back from amounts
        let recovered_liquidity =
            get_liquidity_for_amounts(&env, sqrt_price, sqrt_lower, sqrt_upper, amount0, amount1);

        // Token1 roundtrip should be very precise since it's just mul/div by Q96
        let diff = if recovered_liquidity > initial_liquidity {
            recovered_liquidity - initial_liquidity
        } else {
            initial_liquidity - recovered_liquidity
        };
        // Allow small rounding error (a few units due to Q96 division rounding)
        assert!(
            diff <= 10,
            "Token1 liquidity should roundtrip precisely, got diff: {}, initial: {}",
            diff,
            initial_liquidity
        );
    }

    #[test]
    fn test_liquidity_amounts_consistency() {
        let env = Env::default();
        // Test that get_amounts and get_liquidity are inversely related
        // For in-range positions, get_liquidity_for_amounts takes the minimum
        // of liquidity from token0 and token1, which may differ significantly
        // due to the asymmetric price ranges (current to upper vs lower to current)
        let sqrt_price = Q96;
        let sqrt_lower = Q96 * 9 / 10;
        let sqrt_upper = Q96 * 11 / 10;
        let initial_liquidity = 1_000_000_000_000_000u128;

        let (amount0, amount1) =
            get_amounts_for_liquidity(&env, sqrt_price, sqrt_lower, sqrt_upper, initial_liquidity);

        // Both amounts should be positive for in-range position
        assert!(amount0 > 0, "amount0 should be > 0 in range");
        assert!(amount1 > 0, "amount1 should be > 0 in range");

        let recovered_liquidity =
            get_liquidity_for_amounts(&env, sqrt_price, sqrt_lower, sqrt_upper, amount0, amount1);

        // Recovered liquidity should be positive
        assert!(
            recovered_liquidity > 0,
            "Recovered liquidity should be positive"
        );

        // The key invariant: with the recovered liquidity,
        // the amounts should not exceed what we put in
        let (amount0_check, amount1_check) = get_amounts_for_liquidity(
            &env,
            sqrt_price,
            sqrt_lower,
            sqrt_upper,
            recovered_liquidity,
        );

        assert!(
            amount0_check <= amount0,
            "Recovered amount0 should not exceed original"
        );
        assert!(
            amount1_check <= amount1,
            "Recovered amount1 should not exceed original"
        );
    }

    // === Wide vs narrow range tests ===

    #[test]
    fn test_wider_range_less_liquidity() {
        let env = Env::default();
        let sqrt_price = Q96;
        let amount0 = 1_000_000_000_000u128;
        let amount1 = 1_000_000_000_000u128;

        // Narrow range
        let sqrt_lower_narrow = Q96 * 99 / 100; // 0.99
        let sqrt_upper_narrow = Q96 * 101 / 100; // 1.01
        let liquidity_narrow = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower_narrow,
            sqrt_upper_narrow,
            amount0,
            amount1,
        );

        // Wide range
        let sqrt_lower_wide = Q96 * 8 / 10; // 0.8
        let sqrt_upper_wide = Q96 * 12 / 10; // 1.2
        let liquidity_wide = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower_wide,
            sqrt_upper_wide,
            amount0,
            amount1,
        );

        // Narrow range should provide more liquidity for same capital
        assert!(
            liquidity_narrow > liquidity_wide,
            "Narrow range should provide more liquidity"
        );
    }

    // === Fee tier tick spacing scenarios ===

    #[test]
    fn test_liquidity_at_common_tick_spacings() {
        let env = Env::default();
        let sqrt_price = Q96;
        let amount0 = 1_000_000_000_000u128;
        let amount1 = 1_000_000_000_000u128;

        // 0.05% fee (10 tick spacing)
        let sqrt_lower_10 = Q96 * 999 / 1000;
        let sqrt_upper_10 = Q96 * 1001 / 1000;
        let liquidity_10 = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower_10,
            sqrt_upper_10,
            amount0,
            amount1,
        );
        assert!(liquidity_10 > 0);

        // 0.3% fee (60 tick spacing)
        let sqrt_lower_60 = Q96 * 99 / 100;
        let sqrt_upper_60 = Q96 * 101 / 100;
        let liquidity_60 = get_liquidity_for_amounts(
            &env,
            sqrt_price,
            sqrt_lower_60,
            sqrt_upper_60,
            amount0,
            amount1,
        );
        assert!(liquidity_60 > 0);

        // Smaller range should give more liquidity
        assert!(liquidity_10 > liquidity_60);
    }
}
