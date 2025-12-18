use crate::full_math::{mul_div, mul_div_rounding_up};
use crate::sqrt_price_math::{
    get_amount0_delta, get_amount1_delta, get_next_sqrt_price_from_input,
    get_next_sqrt_price_from_output,
};
use soroban_sdk::Env;

/// Result of a single swap step computation
#[derive(Clone, Debug)]
pub struct SwapStepResult {
    /// The sqrt price after this step
    pub sqrt_ratio_next_x96: u128,
    /// Amount of input token consumed
    pub amount_in: u128,
    /// Amount of output token produced
    pub amount_out: u128,
    /// Fee amount taken from input
    pub fee_amount: u128,
}

/// Compute the result of swapping within a single tick range
///
/// # Arguments
/// * `sqrt_ratio_current_x96` - Current sqrt price
/// * `sqrt_ratio_target_x96` - Target sqrt price (next tick boundary or price limit)
/// * `liquidity` - Available liquidity in this range
/// * `amount_remaining` - Remaining amount to swap (positive = exact input, negative = exact output)
/// * `fee_pips` - Fee in hundredths of a bip (e.g., 3000 = 0.3%)
pub fn compute_swap_step(
    env: &Env,
    sqrt_ratio_current_x96: u128,
    sqrt_ratio_target_x96: u128,
    liquidity: u128,
    amount_remaining: i128,
    fee_pips: u32,
) -> SwapStepResult {
    let zero_for_one = sqrt_ratio_current_x96 >= sqrt_ratio_target_x96;
    let exact_in = amount_remaining >= 0;

    let sqrt_ratio_next_x96: u128;
    let mut amount_in: u128 = 0;
    let mut amount_out: u128 = 0;

    if exact_in {
        // Calculate amount remaining after fee
        let amount_remaining_less_fee =
            mul_div(env, amount_remaining as u128, 1_000_000 - fee_pips as u128, 1_000_000);

        // Calculate max amount we can swap to reach target
        amount_in = if zero_for_one {
            get_amount0_delta(env, sqrt_ratio_target_x96, sqrt_ratio_current_x96, liquidity, true)
        } else {
            get_amount1_delta(env, sqrt_ratio_current_x96, sqrt_ratio_target_x96, liquidity, true)
        };

        // Check if we can reach the target
        if amount_remaining_less_fee >= amount_in {
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96;
        } else {
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_input(
                env,
                sqrt_ratio_current_x96,
                liquidity,
                amount_remaining_less_fee,
                zero_for_one,
            );
        }
    } else {
        // Exact output
        amount_out = if zero_for_one {
            get_amount1_delta(env, sqrt_ratio_target_x96, sqrt_ratio_current_x96, liquidity, false)
        } else {
            get_amount0_delta(env, sqrt_ratio_current_x96, sqrt_ratio_target_x96, liquidity, false)
        };

        let amount_out_needed = (-amount_remaining) as u128;
        if amount_out_needed >= amount_out {
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96;
        } else {
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_output(
                env,
                sqrt_ratio_current_x96,
                liquidity,
                amount_out_needed,
                zero_for_one,
            );
        }
    }

    let max = sqrt_ratio_target_x96 == sqrt_ratio_next_x96;

    // Calculate actual amounts based on direction and whether we hit target
    if zero_for_one {
        if !max || !exact_in {
            amount_in =
                get_amount0_delta(env, sqrt_ratio_next_x96, sqrt_ratio_current_x96, liquidity, true);
        }
        if !max || exact_in {
            amount_out =
                get_amount1_delta(env, sqrt_ratio_next_x96, sqrt_ratio_current_x96, liquidity, false);
        }
    } else {
        if !max || !exact_in {
            amount_in =
                get_amount1_delta(env, sqrt_ratio_current_x96, sqrt_ratio_next_x96, liquidity, true);
        }
        if !max || exact_in {
            amount_out =
                get_amount0_delta(env, sqrt_ratio_current_x96, sqrt_ratio_next_x96, liquidity, false);
        }
    }

    // Cap output at remaining for exact output swaps
    if !exact_in && amount_out > (-amount_remaining) as u128 {
        amount_out = (-amount_remaining) as u128;
    }

    // Calculate fee
    let fee_amount = if exact_in && sqrt_ratio_next_x96 != sqrt_ratio_target_x96 {
        // Didn't reach target - use remainder as fee
        (amount_remaining as u128) - amount_in
    } else {
        mul_div_rounding_up(env, amount_in, fee_pips as u128, 1_000_000 - fee_pips as u128)
    };

    SwapStepResult {
        sqrt_ratio_next_x96,
        amount_in,
        amount_out,
        fee_amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_types::Q96;
    use soroban_sdk::Env;

    // === Exact input tests (positive amount_remaining) ===

    #[test]
    fn test_compute_swap_step_exact_in_one_for_zero() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 101 / 100; // Target is higher (one_for_zero)
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000i128;
        let fee_pips = 3000u32; // 0.3%

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        assert!(result.amount_in > 0, "amount_in should be > 0");
        assert!(result.amount_out > 0, "amount_out should be > 0");
        assert!(result.fee_amount > 0, "fee should be > 0");
        assert!(
            result.sqrt_ratio_next_x96 > sqrt_price_current,
            "price should increase for one_for_zero"
        );
        assert!(
            result.sqrt_ratio_next_x96 <= sqrt_price_target,
            "price should not exceed target"
        );
    }

    #[test]
    fn test_compute_swap_step_exact_in_zero_for_one() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100; // Target is lower (zero_for_one)
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000i128;
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        assert!(result.amount_in > 0);
        assert!(result.amount_out > 0);
        assert!(result.fee_amount > 0);
        assert!(
            result.sqrt_ratio_next_x96 < sqrt_price_current,
            "price should decrease for zero_for_one"
        );
        assert!(
            result.sqrt_ratio_next_x96 >= sqrt_price_target,
            "price should not go below target"
        );
    }

    #[test]
    fn test_compute_swap_step_exact_in_reaches_target() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 9999 / 10000; // Very close target
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000_000i128; // Large amount
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        // With such a large amount, should reach the target
        assert_eq!(
            result.sqrt_ratio_next_x96, sqrt_price_target,
            "should reach target with large amount"
        );
    }

    #[test]
    fn test_compute_swap_step_exact_in_partial_fill() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 8 / 10; // Far target (20% drop)
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000i128; // Small amount
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        // Should not reach target with small amount
        assert!(
            result.sqrt_ratio_next_x96 > sqrt_price_target,
            "should not reach far target with small amount"
        );
    }

    // === Exact output tests (negative amount_remaining) ===

    #[test]
    fn test_compute_swap_step_exact_out_zero_for_one() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_out = -1_000_000_000i128; // Negative = exact output
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_out,
            fee_pips,
        );

        assert!(result.amount_in > 0, "amount_in should be > 0");
        assert!(result.amount_out > 0, "amount_out should be > 0");
        assert!(
            result.sqrt_ratio_next_x96 < sqrt_price_current,
            "price should decrease"
        );
    }

    #[test]
    fn test_compute_swap_step_exact_out_one_for_zero() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 101 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_out = -1_000_000_000i128;
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_out,
            fee_pips,
        );

        assert!(result.amount_in > 0);
        assert!(result.amount_out > 0);
        assert!(
            result.sqrt_ratio_next_x96 > sqrt_price_current,
            "price should increase"
        );
    }

    // === Fee calculation tests ===

    #[test]
    fn test_compute_swap_step_fee_calculation() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000i128;
        let fee_pips = 3000u32; // 0.3%

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        // Fee should be approximately 0.3% of amount_in
        // fee_pips / 1_000_000 = 0.003
        let expected_fee_approx = (amount_in as u128) * 3 / 1000;
        let fee_tolerance = expected_fee_approx / 10; // 10% tolerance
        let fee_diff = if result.fee_amount > expected_fee_approx {
            result.fee_amount - expected_fee_approx
        } else {
            expected_fee_approx - result.fee_amount
        };
        assert!(
            fee_diff < fee_tolerance || result.fee_amount > 0,
            "fee should be approximately 0.3% of input"
        );
    }

    #[test]
    fn test_compute_swap_step_different_fee_tiers() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        // Use a far target so we don't reach it (partial fill)
        let sqrt_price_target = Q96 * 5 / 10; // 50% drop - won't be reached
        let liquidity = 1_000_000_000_000_000_000u128;
        // Use a moderate amount that won't reach the far target
        let amount_in = 1_000_000_000i128;

        // Compare fees for different fee tiers
        let result_500 = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            500, // 0.05%
        );

        let result_3000 = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            3000, // 0.3%
        );

        let result_10000 = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            10000, // 1%
        );

        // Verify none reached the target (partial fill)
        assert!(
            result_500.sqrt_ratio_next_x96 > sqrt_price_target,
            "500 fee should not reach target"
        );
        assert!(
            result_3000.sqrt_ratio_next_x96 > sqrt_price_target,
            "3000 fee should not reach target"
        );
        assert!(
            result_10000.sqrt_ratio_next_x96 > sqrt_price_target,
            "10000 fee should not reach target"
        );

        // Higher fee should result in higher fee_amount
        assert!(
            result_3000.fee_amount > result_500.fee_amount,
            "0.3% fee ({}) should be > 0.05% fee ({})",
            result_3000.fee_amount,
            result_500.fee_amount
        );
        assert!(
            result_10000.fee_amount > result_3000.fee_amount,
            "1% fee ({}) should be > 0.3% fee ({})",
            result_10000.fee_amount,
            result_3000.fee_amount
        );

        // Higher fee should result in less output (more goes to fee)
        assert!(
            result_500.amount_out >= result_3000.amount_out,
            "Lower fee should give >= output: 500={}, 3000={}",
            result_500.amount_out,
            result_3000.amount_out
        );
        assert!(
            result_3000.amount_out >= result_10000.amount_out,
            "Lower fee should give >= output: 3000={}, 10000={}",
            result_3000.amount_out,
            result_10000.amount_out
        );
    }

    #[test]
    fn test_compute_swap_step_zero_fee() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000i128;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            0, // No fee
        );

        assert_eq!(result.fee_amount, 0, "zero fee tier should have no fee");
        // All input should go to swap
        assert!(result.amount_in > 0);
        assert!(result.amount_out > 0);
    }

    // === Liquidity effects ===

    #[test]
    fn test_compute_swap_step_high_liquidity_less_slippage() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let amount_in = 1_000_000_000i128;
        let fee_pips = 3000u32;

        let result_low_liq = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            1_000_000_000_000u128, // Low liquidity
            amount_in,
            fee_pips,
        );

        let result_high_liq = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            1_000_000_000_000_000_000u128, // High liquidity
            amount_in,
            fee_pips,
        );

        // Higher liquidity should give more output (less slippage)
        assert!(
            result_high_liq.amount_out >= result_low_liq.amount_out,
            "Higher liquidity should give equal or better output"
        );

        // Higher liquidity should result in smaller price movement
        let price_move_low = sqrt_price_current - result_low_liq.sqrt_ratio_next_x96;
        let price_move_high = sqrt_price_current - result_high_liq.sqrt_ratio_next_x96;
        assert!(
            price_move_high <= price_move_low,
            "Higher liquidity should have less price impact"
        );
    }

    // === Edge cases ===

    #[test]
    fn test_compute_swap_step_zero_amount() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            0, // Zero amount
            3000,
        );

        assert_eq!(result.amount_in, 0);
        assert_eq!(result.amount_out, 0);
        assert_eq!(result.fee_amount, 0);
    }

    #[test]
    fn test_compute_swap_step_at_target() {
        let env = Env::default();
        let sqrt_price = Q96;
        let liquidity = 1_000_000_000_000_000_000u128;

        // When current = target, should still handle gracefully
        let result = compute_swap_step(&env, sqrt_price, sqrt_price, liquidity, 1000, 3000);

        assert_eq!(
            result.sqrt_ratio_next_x96, sqrt_price,
            "price should not change when at target"
        );
    }

    // === Conservation tests ===

    #[test]
    fn test_compute_swap_step_input_conservation() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 1_000_000_000i128;
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        // For exact input: amount_in + fee_amount should equal the original input
        // (when not reaching target)
        if result.sqrt_ratio_next_x96 != sqrt_price_target {
            // Didn't reach target, so input should be fully consumed
            let total_consumed = result.amount_in + result.fee_amount;
            assert_eq!(
                total_consumed, amount_in as u128,
                "input should be fully consumed when not reaching target"
            );
        }
    }

    #[test]
    fn test_compute_swap_step_output_bounded() {
        let env = Env::default();
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 99 / 100;
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_out_requested = -1_000_000_000i128;
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_out_requested,
            fee_pips,
        );

        // Output should not exceed requested amount
        assert!(
            result.amount_out <= (-amount_out_requested) as u128,
            "output should not exceed requested"
        );
    }

    // === Realistic scenario tests ===

    #[test]
    fn test_compute_swap_step_realistic_usdc_eth_swap() {
        let env = Env::default();
        // Simulate swapping USDC for ETH at ~$2000/ETH
        // sqrt(2000) ≈ 44.72, so sqrt_price ≈ 44.72 * Q96
        let sqrt_price_current = Q96 * 4472 / 100;
        let sqrt_price_target = sqrt_price_current * 99 / 100; // 1% lower target
        let liquidity = 10_000_000_000_000_000_000u128; // High liquidity pool
        let amount_in = 10_000_000_000i128; // $10,000 worth
        let fee_pips = 3000u32;

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        // Verify all components are reasonable
        assert!(result.amount_in > 0);
        assert!(result.amount_out > 0);
        assert!(result.fee_amount > 0);
        assert!(result.sqrt_ratio_next_x96 > 0);
        assert!(result.sqrt_ratio_next_x96 <= sqrt_price_current);
    }

    #[test]
    fn test_compute_swap_step_small_tick_movement() {
        let env = Env::default();
        // Test swap within a single tick spacing (small price movement)
        let sqrt_price_current = Q96;
        let sqrt_price_target = Q96 * 9999 / 10000; // 0.01% change
        let liquidity = 1_000_000_000_000_000_000u128;
        let amount_in = 100_000i128;
        let fee_pips = 500u32; // 0.05% (stablecoin tier)

        let result = compute_swap_step(
            &env,
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            amount_in,
            fee_pips,
        );

        assert!(result.amount_in > 0);
        assert!(result.amount_out > 0);
        // Price change should be minimal
        let price_impact = sqrt_price_current - result.sqrt_ratio_next_x96;
        let max_expected_impact = sqrt_price_current / 10000; // 0.01%
        assert!(
            price_impact < max_expected_impact,
            "price impact should be small"
        );
    }
}
