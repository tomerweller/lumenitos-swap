use soroban_sdk::{Env, U256};

/// Multiply and divide with 256-bit intermediate precision (rounds down)
/// Returns (a * b) / denominator
pub fn mul_div(env: &Env, a: u128, b: u128, denominator: u128) -> u128 {
    if denominator == 0 {
        panic!("Division by zero");
    }

    let a_256 = U256::from_u128(env, a);
    let b_256 = U256::from_u128(env, b);
    let denom_256 = U256::from_u128(env, denominator);

    // (a * b) / denominator using fixed_div_floor
    // First multiply a * b, then divide by denominator
    let product = a_256.mul(&b_256);
    let result = product.div(&denom_256);

    // Convert back to u128
    u128_from_u256(env, &result)
}

/// Multiply and divide with 256-bit intermediate precision (rounds up)
/// Returns ceil((a * b) / denominator)
pub fn mul_div_rounding_up(env: &Env, a: u128, b: u128, denominator: u128) -> u128 {
    let result = mul_div(env, a, b, denominator);

    // Check if there was a remainder
    let a_256 = U256::from_u128(env, a);
    let b_256 = U256::from_u128(env, b);
    let denom_256 = U256::from_u128(env, denominator);

    let product = a_256.mul(&b_256);
    let remainder = product.rem_euclid(&denom_256);

    if remainder.gt(&U256::from_u32(env, 0)) {
        result + 1
    } else {
        result
    }
}

/// Convert U256 to u128, panics if overflow
fn u128_from_u256(env: &Env, value: &U256) -> u128 {
    let max_u128 = U256::from_u128(env, u128::MAX);
    if value.gt(&max_u128) {
        panic!("U256 overflow when converting to u128");
    }
    value.to_u128().unwrap()
}

/// Unsigned division with rounding up
pub fn div_rounding_up(a: u128, b: u128) -> u128 {
    if b == 0 {
        panic!("Division by zero");
    }
    if a == 0 {
        return 0;
    }
    (a - 1) / b + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    // === mul_div tests ===

    #[test]
    fn test_mul_div_basic() {
        let env = Env::default();
        // Basic test: (10 * 20) / 5 = 40
        assert_eq!(mul_div(&env, 10, 20, 5), 40);
    }

    #[test]
    fn test_mul_div_large_numbers() {
        let env = Env::default();
        // Test with larger numbers that would overflow u128
        // (2^100 * 2^100) / 2^100 = 2^100
        let large = 1u128 << 100;
        assert_eq!(mul_div(&env, large, large, large), large);
    }

    #[test]
    fn test_mul_div_max_values() {
        let env = Env::default();
        // (MAX * MAX) / MAX = MAX (should work with U256 intermediate)
        let max = u128::MAX;
        assert_eq!(mul_div(&env, max, max, max), max);
    }

    #[test]
    fn test_mul_div_zero_numerator() {
        let env = Env::default();
        assert_eq!(mul_div(&env, 0, 100, 50), 0);
        assert_eq!(mul_div(&env, 100, 0, 50), 0);
    }

    #[test]
    fn test_mul_div_rounds_down() {
        let env = Env::default();
        // 1 * 1 / 2 = 0 (rounds down)
        assert_eq!(mul_div(&env, 1, 1, 2), 0);
        // 3 * 1 / 2 = 1 (rounds down from 1.5)
        assert_eq!(mul_div(&env, 3, 1, 2), 1);
        // 5 * 1 / 3 = 1 (rounds down from 1.67)
        assert_eq!(mul_div(&env, 5, 1, 3), 1);
    }

    #[test]
    fn test_mul_div_q96_operations() {
        let env = Env::default();
        // Simulate Q96 price operations
        let q96 = 1u128 << 96;
        // 1 * Q96 / Q96 = 1
        assert_eq!(mul_div(&env, 1, q96, q96), 1);
        // Q96 * Q96 / Q96 = Q96
        assert_eq!(mul_div(&env, q96, q96, q96), q96);
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_mul_div_zero_denominator() {
        let env = Env::default();
        mul_div(&env, 10, 20, 0);
    }

    // === mul_div_rounding_up tests ===

    #[test]
    fn test_mul_div_rounding_up_exact() {
        let env = Env::default();
        // Exact division: (10 * 20) / 5 = 40
        assert_eq!(mul_div_rounding_up(&env, 10, 20, 5), 40);
    }

    #[test]
    fn test_mul_div_rounding_up_with_remainder() {
        let env = Env::default();
        // With remainder: (10 * 3) / 7 = 4.28... -> 5
        assert_eq!(mul_div_rounding_up(&env, 10, 3, 7), 5);
        // 1 * 1 / 2 = 0.5 -> 1
        assert_eq!(mul_div_rounding_up(&env, 1, 1, 2), 1);
        // 1 * 1 / 3 = 0.33 -> 1
        assert_eq!(mul_div_rounding_up(&env, 1, 1, 3), 1);
    }

    #[test]
    fn test_mul_div_rounding_up_large_numbers() {
        let env = Env::default();
        let large = 1u128 << 100;
        // (large * large + 1) / large should round up
        assert_eq!(mul_div_rounding_up(&env, large, large, large), large);
    }

    #[test]
    fn test_mul_div_rounding_up_vs_down_difference() {
        let env = Env::default();
        // When there's a remainder, rounding up should be exactly 1 more than rounding down
        let result_down = mul_div(&env, 7, 11, 13);
        let result_up = mul_div_rounding_up(&env, 7, 11, 13);
        // 7 * 11 = 77, 77 / 13 = 5.923... -> down: 5, up: 6
        assert_eq!(result_down, 5);
        assert_eq!(result_up, 6);
        assert_eq!(result_up - result_down, 1);
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_mul_div_rounding_up_zero_denominator() {
        let env = Env::default();
        mul_div_rounding_up(&env, 10, 20, 0);
    }

    // === div_rounding_up tests ===

    #[test]
    fn test_div_rounding_up_exact() {
        assert_eq!(div_rounding_up(9, 3), 3); // 9/3 = 3 exactly
        assert_eq!(div_rounding_up(100, 10), 10);
    }

    #[test]
    fn test_div_rounding_up_with_remainder() {
        assert_eq!(div_rounding_up(10, 3), 4); // 10/3 = 3.33 -> 4
        assert_eq!(div_rounding_up(11, 3), 4); // 11/3 = 3.67 -> 4
        assert_eq!(div_rounding_up(1, 2), 1); // 1/2 = 0.5 -> 1
    }

    #[test]
    fn test_div_rounding_up_zero_numerator() {
        assert_eq!(div_rounding_up(0, 5), 0);
        assert_eq!(div_rounding_up(0, 1), 0);
    }

    #[test]
    fn test_div_rounding_up_large_numbers() {
        let large = u128::MAX - 1;
        assert_eq!(div_rounding_up(large, large), 1);
        assert_eq!(div_rounding_up(large, 1), large);
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_div_rounding_up_zero_denominator() {
        div_rounding_up(10, 0);
    }

    // === Uniswap v3 specific test cases ===

    #[test]
    fn test_phantom_overflow_scenario() {
        let env = Env::default();
        // Test case from Uniswap v3: should handle phantom overflow
        // When a * b overflows u128 but result fits in u128
        let q128 = 1u128 << 64;
        let a = q128 * 3;
        let b = q128 * 2;
        let denom = q128;
        // (3 * 2^64) * (2 * 2^64) / 2^64 = 6 * 2^64
        let result = mul_div(&env, a, b, denom);
        assert_eq!(result, q128 * 6);
    }

    #[test]
    fn test_accuracy_at_large_scale() {
        let env = Env::default();
        // Test that we maintain precision with Q96 arithmetic
        let q96 = 1u128 << 96;
        let price = q96 + (q96 / 1000); // 1.001 in Q96
        let amount = 1_000_000_000_000u128; // 1 trillion

        let result = mul_div(&env, amount, price, q96);
        // Expected: ~1001000000000 (1 trillion * 1.001)
        // Allow for small rounding error (1 unit tolerance)
        let expected = 1_001_000_000_000u128;
        let diff = if result > expected {
            result - expected
        } else {
            expected - result
        };
        assert!(diff <= 1, "Result should be within 1 of expected");
    }
}
