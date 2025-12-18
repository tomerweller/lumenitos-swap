use crate::full_math::{div_rounding_up, mul_div, mul_div_rounding_up};
use dex_types::Q96;
use soroban_sdk::{Env, U256};

/// Calculate amount0 delta for a price move from sqrt_ratio_a to sqrt_ratio_b
/// delta_x = L * (sqrt_pb - sqrt_pa) / (sqrt_pa * sqrt_pb)
pub fn get_amount0_delta(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    let numerator1 = (liquidity as u128) << 96;
    let numerator2 = sqrt_ratio_upper - sqrt_ratio_lower;

    if sqrt_ratio_lower == 0 {
        panic!("sqrt_ratio_lower cannot be zero");
    }

    if round_up {
        div_rounding_up(
            mul_div_rounding_up(env, numerator1, numerator2, sqrt_ratio_upper),
            sqrt_ratio_lower,
        )
    } else {
        mul_div(env, numerator1, numerator2, sqrt_ratio_upper) / sqrt_ratio_lower
    }
}

/// Calculate amount1 delta for a price move from sqrt_ratio_a to sqrt_ratio_b
/// delta_y = L * (sqrt_pb - sqrt_pa)
pub fn get_amount1_delta(
    env: &Env,
    sqrt_ratio_a_x96: u128,
    sqrt_ratio_b_x96: u128,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    let (sqrt_ratio_lower, sqrt_ratio_upper) = if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_b_x96, sqrt_ratio_a_x96)
    } else {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96)
    };

    if round_up {
        mul_div_rounding_up(env, liquidity, sqrt_ratio_upper - sqrt_ratio_lower, Q96)
    } else {
        mul_div(env, liquidity, sqrt_ratio_upper - sqrt_ratio_lower, Q96)
    }
}

/// Get next sqrt price from an input amount of token0 or token1
pub fn get_next_sqrt_price_from_input(
    env: &Env,
    sqrt_price_x96: u128,
    liquidity: u128,
    amount_in: u128,
    zero_for_one: bool,
) -> u128 {
    if sqrt_price_x96 == 0 || liquidity == 0 {
        panic!("Invalid inputs");
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount0_rounding_up(env, sqrt_price_x96, liquidity, amount_in, true)
    } else {
        get_next_sqrt_price_from_amount1_rounding_down(env, sqrt_price_x96, liquidity, amount_in, true)
    }
}

/// Get next sqrt price from an output amount
pub fn get_next_sqrt_price_from_output(
    env: &Env,
    sqrt_price_x96: u128,
    liquidity: u128,
    amount_out: u128,
    zero_for_one: bool,
) -> u128 {
    if sqrt_price_x96 == 0 || liquidity == 0 {
        panic!("Invalid inputs");
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount1_rounding_down(env, sqrt_price_x96, liquidity, amount_out, false)
    } else {
        get_next_sqrt_price_from_amount0_rounding_up(env, sqrt_price_x96, liquidity, amount_out, false)
    }
}

/// Calculate next sqrt price given a token0 amount
/// sqrt_price_next = sqrt_price * L / (L + amount * sqrt_price)  [if add]
/// sqrt_price_next = sqrt_price * L / (L - amount * sqrt_price)  [if remove]
fn get_next_sqrt_price_from_amount0_rounding_up(
    env: &Env,
    sqrt_price_x96: u128,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> u128 {
    if amount == 0 {
        return sqrt_price_x96;
    }

    let numerator1 = (liquidity as u128) << 96;

    if add {
        let product = amount.checked_mul(sqrt_price_x96);
        if let Some(product) = product {
            let denominator = numerator1.checked_add(product);
            if let Some(denominator) = denominator {
                if denominator >= numerator1 {
                    return mul_div_rounding_up(env, numerator1, sqrt_price_x96, denominator);
                }
            }
        }
        // Fallback to U256 calculation
        let numerator1_256 = U256::from_u128(env, numerator1);
        let sqrt_price_256 = U256::from_u128(env, sqrt_price_x96);
        let amount_256 = U256::from_u128(env, amount);

        let product = amount_256.mul(&sqrt_price_256);
        let denominator = numerator1_256.add(&product);
        let result = numerator1_256.mul(&sqrt_price_256).div(&denominator);

        // Add 1 for rounding up
        result.to_u128().unwrap() + 1
    } else {
        let product = amount.checked_mul(sqrt_price_x96);
        if let Some(product) = product {
            if numerator1 > product {
                let denominator = numerator1 - product;
                return mul_div_rounding_up(env, numerator1, sqrt_price_x96, denominator);
            }
        }
        panic!("Denominator underflow");
    }
}

/// Calculate next sqrt price given a token1 amount
/// sqrt_price_next = sqrt_price + amount / L  [if add]
/// sqrt_price_next = sqrt_price - amount / L  [if remove]
fn get_next_sqrt_price_from_amount1_rounding_down(
    env: &Env,
    sqrt_price_x96: u128,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> u128 {
    if add {
        let quotient = if amount <= u128::MAX >> 96 {
            (amount << 96) / liquidity
        } else {
            mul_div(env, amount, Q96, liquidity)
        };
        sqrt_price_x96 + quotient
    } else {
        let quotient = if amount <= u128::MAX >> 96 {
            div_rounding_up(amount << 96, liquidity)
        } else {
            mul_div_rounding_up(env, amount, Q96, liquidity)
        };
        if sqrt_price_x96 <= quotient {
            panic!("sqrt_price underflow");
        }
        sqrt_price_x96 - quotient
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};
    use soroban_sdk::Env;

    // === get_amount0_delta tests ===

    #[test]
    fn test_get_amount0_delta_basic() {
        let env = Env::default();
        let sqrt_a = Q96; // sqrt(1) * 2^96
        let sqrt_b = Q96 * 2; // sqrt(4) * 2^96 (approximately)
        let liquidity = 1_000_000_000_000u128; // 1e12

        let amount0 = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        assert!(amount0 > 0);
    }

    #[test]
    fn test_get_amount0_delta_order_independent() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 * 11 / 10; // 1.1
        let liquidity = 1_000_000_000_000u128;

        let amount_ab = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        let amount_ba = get_amount0_delta(&env, sqrt_b, sqrt_a, liquidity, false);

        // Order shouldn't matter
        assert_eq!(amount_ab, amount_ba);
    }

    #[test]
    fn test_get_amount0_delta_zero_range() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000u128;

        let amount = get_amount0_delta(&env, sqrt_price, sqrt_price, liquidity, false);
        assert_eq!(amount, 0, "Zero price range should give zero amount");
    }

    #[test]
    fn test_get_amount0_delta_rounding() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 + Q96 / 100; // Small price change
        let liquidity = 1_000_000_000u128;

        let amount_down = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        let amount_up = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, true);

        // Rounding up should give >= rounding down
        assert!(amount_up >= amount_down);
        // Difference should be at most 1
        assert!(amount_up - amount_down <= 1);
    }

    #[test]
    fn test_get_amount0_delta_large_liquidity() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 * 11 / 10;
        let liquidity = 1_000_000_000_000_000_000u128; // 1e18

        let amount = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        assert!(amount > 0);
        // Larger liquidity should give larger amount
        let small_liquidity = 1_000_000_000u128;
        let small_amount = get_amount0_delta(&env, sqrt_a, sqrt_b, small_liquidity, false);
        assert!(amount > small_amount);
    }

    #[test]
    #[should_panic(expected = "sqrt_ratio_lower cannot be zero")]
    fn test_get_amount0_delta_zero_sqrt_ratio() {
        let env = Env::default();
        get_amount0_delta(&env, 0, Q96, 1000, false);
    }

    // === get_amount1_delta tests ===

    #[test]
    fn test_get_amount1_delta_basic() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 * 2;
        let liquidity = 1_000_000_000_000u128;

        let amount1 = get_amount1_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        assert!(amount1 > 0);
    }

    #[test]
    fn test_get_amount1_delta_order_independent() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 * 11 / 10;
        let liquidity = 1_000_000_000_000u128;

        let amount_ab = get_amount1_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        let amount_ba = get_amount1_delta(&env, sqrt_b, sqrt_a, liquidity, false);

        assert_eq!(amount_ab, amount_ba);
    }

    #[test]
    fn test_get_amount1_delta_zero_range() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000u128;

        let amount = get_amount1_delta(&env, sqrt_price, sqrt_price, liquidity, false);
        assert_eq!(amount, 0);
    }

    #[test]
    fn test_get_amount1_delta_rounding() {
        let env = Env::default();
        let sqrt_a = Q96;
        let sqrt_b = Q96 + Q96 / 100;
        let liquidity = 1_000_000_000u128;

        let amount_down = get_amount1_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        let amount_up = get_amount1_delta(&env, sqrt_a, sqrt_b, liquidity, true);

        assert!(amount_up >= amount_down);
        assert!(amount_up - amount_down <= 1);
    }

    // === get_next_sqrt_price_from_input tests ===

    #[test]
    fn test_get_next_sqrt_price_from_input_zero_for_one() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000u128;

        let next_sqrt = get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, amount_in, true);

        // When selling token0 (zero_for_one), price should decrease
        assert!(next_sqrt < sqrt_price, "zero_for_one should decrease sqrt price");
    }

    #[test]
    fn test_get_next_sqrt_price_from_input_one_for_zero() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000u128;

        let next_sqrt = get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, amount_in, false);

        // When selling token1 (one_for_zero), price should increase
        assert!(next_sqrt > sqrt_price, "one_for_zero should increase sqrt price");
    }

    #[test]
    fn test_get_next_sqrt_price_from_input_zero_amount() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000u128;

        let next_sqrt = get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, 0, true);
        assert_eq!(next_sqrt, sqrt_price, "Zero input should not change price");
    }

    #[test]
    fn test_get_next_sqrt_price_from_input_large_amount() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let small_amount = 1_000_000u128;
        let large_amount = 1_000_000_000_000u128;

        let next_sqrt_small =
            get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, small_amount, true);
        let next_sqrt_large =
            get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, large_amount, true);

        // Larger input should move price more
        assert!(next_sqrt_large < next_sqrt_small);
    }

    #[test]
    #[should_panic(expected = "Invalid inputs")]
    fn test_get_next_sqrt_price_from_input_zero_liquidity() {
        let env = Env::default();
        get_next_sqrt_price_from_input(&env, Q96, 0, 1000, true);
    }

    #[test]
    #[should_panic(expected = "Invalid inputs")]
    fn test_get_next_sqrt_price_from_input_zero_sqrt_price() {
        let env = Env::default();
        get_next_sqrt_price_from_input(&env, 0, 1000, 1000, true);
    }

    // === get_next_sqrt_price_from_output tests ===

    #[test]
    fn test_get_next_sqrt_price_from_output_zero_for_one() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_out = 1_000_000_000u128;

        let next_sqrt = get_next_sqrt_price_from_output(&env, sqrt_price, liquidity, amount_out, true);

        // zero_for_one (buying token1) should decrease sqrt price
        assert!(next_sqrt < sqrt_price);
    }

    #[test]
    fn test_get_next_sqrt_price_from_output_one_for_zero() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_out = 1_000_000_000u128;

        let next_sqrt = get_next_sqrt_price_from_output(&env, sqrt_price, liquidity, amount_out, false);

        // one_for_zero (buying token0) should increase sqrt price
        assert!(next_sqrt > sqrt_price);
    }

    // === Amount delta formula verification ===

    #[test]
    fn test_amount_deltas_inverse_relationship() {
        let env = Env::default();
        // For a given price range and liquidity:
        // amount0 = L * (1/sqrt_pa - 1/sqrt_pb)
        // amount1 = L * (sqrt_pb - sqrt_pa)
        // These should be consistent with each other

        let sqrt_a = Q96; // price = 1
        let sqrt_b = Q96 * 12 / 10; // price = 1.44
        let liquidity = 1_000_000_000_000_000u128;

        let amount0 = get_amount0_delta(&env, sqrt_a, sqrt_b, liquidity, false);
        let amount1 = get_amount1_delta(&env, sqrt_a, sqrt_b, liquidity, false);

        // Both should be positive
        assert!(amount0 > 0);
        assert!(amount1 > 0);

        // For a 20% sqrt price increase (44% price increase),
        // amount1 should be larger than amount0 (more token1 needed)
        // This is because token1 is the quote token at higher prices
        assert!(amount1 > amount0);
    }

    #[test]
    fn test_price_impact_symmetry() {
        let env = Env::default();
        // Adding amount0 and then removing the same should return to original price (approximately)
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000u128;

        // Move price down
        let sqrt_after = get_next_sqrt_price_from_input(&env, sqrt_price, liquidity, amount_in, true);

        // Calculate how much amount0 this price move represents
        let amount0_delta = get_amount0_delta(&env, sqrt_after, sqrt_price, liquidity, false);

        // Should be approximately equal to amount_in (with some rounding)
        let diff = if amount0_delta > amount_in {
            amount0_delta - amount_in
        } else {
            amount_in - amount0_delta
        };
        // Allow 1% tolerance
        assert!(diff < amount_in / 100, "Amount should be consistent with price move");
    }

    // === Edge cases ===

    #[test]
    fn test_amounts_at_extreme_prices() {
        let env = Env::default();
        let liquidity = 1_000_000_000_000u128;

        // Near minimum price
        let near_min = MIN_SQRT_RATIO * 10;
        let slightly_higher = near_min * 11 / 10;
        let amount0_low = get_amount0_delta(&env, near_min, slightly_higher, liquidity, false);
        let amount1_low = get_amount1_delta(&env, near_min, slightly_higher, liquidity, false);
        assert!(amount0_low > 0 || amount1_low > 0);

        // Near maximum price
        let near_max = MAX_SQRT_RATIO / 10;
        let slightly_lower = near_max * 9 / 10;
        let amount0_high = get_amount0_delta(&env, slightly_lower, near_max, liquidity, false);
        let amount1_high = get_amount1_delta(&env, slightly_lower, near_max, liquidity, false);
        assert!(amount0_high > 0 || amount1_high > 0);
    }

    #[test]
    fn test_small_price_movement() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;

        // Very small price movement (1 wei of sqrt price)
        let next_sqrt = sqrt_price + 1;
        let amount0 = get_amount0_delta(&env, sqrt_price, next_sqrt, liquidity, true);
        let amount1 = get_amount1_delta(&env, sqrt_price, next_sqrt, liquidity, true);

        // Should be very small amounts (possibly 0 due to rounding)
        assert!(amount0 <= 1);
        assert!(amount1 <= 1);
    }
}
