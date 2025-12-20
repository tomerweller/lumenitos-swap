// ============================================================================
// SWAP MODULE - Refactored for Formal Verification
// ============================================================================
//
// This module separates pure computation from side effects:
//
// 1. PURE FUNCTIONS (formally verifiable):
//    - validate_swap_params: Validates input parameters
//    - compute_step_target_price: Determines target price for a step
//    - compute_swap_step_amounts: Computes amounts for a single step
//    - compute_fee_growth_delta: Computes fee growth change
//    - compute_next_tick: Determines tick after price change
//    - compute_final_amounts: Computes final token amounts
//
// 2. SIDE EFFECT FUNCTIONS:
//    - apply_swap_to_state: Updates pool state in storage
//    - apply_tick_crossing: Updates tick storage when crossing
//    - transfer_swap_tokens: Handles token transfers
//
// 3. ORCHESTRATION:
//    - execute_swap: Main entry point that orchestrates pure + effects
//
// ============================================================================

use crate::storage::{get_config, get_state, set_state, MAX_TICK_CROSSINGS_PER_SWAP};
use crate::tick::{cross, next_initialized_tick_within_one_word};
use dex_math::{add_delta, compute_swap_step, get_sqrt_ratio_at_tick};
use dex_types::{
    SwapComputation, SwapParams, SwapState, MAX_SQRT_RATIO, MIN_SQRT_RATIO,
};
use soroban_sdk::{Address, Env};

#[cfg(feature = "certora")]
use crate::token as token;

#[cfg(not(feature = "certora"))]
use soroban_sdk::token as token;

// ============================================================================
// PURE FUNCTIONS - No storage access, formally verifiable
// ============================================================================

/// Validate swap parameters (pure)
/// Returns the effective sqrt price limit
/// Panics if parameters are invalid
pub fn validate_swap_params(
    amount_specified: i128,
    zero_for_one: bool,
    sqrt_price_limit_x96: u128,
    current_sqrt_price_x96: u128,
) -> u128 {
    if amount_specified == 0 {
        panic!("Amount must be non-zero");
    }

    // Determine effective price limit
    let sqrt_price_limit = if sqrt_price_limit_x96 == 0 {
        if zero_for_one {
            MIN_SQRT_RATIO + 1
        } else {
            MAX_SQRT_RATIO - 1
        }
    } else {
        sqrt_price_limit_x96
    };

    // Validate price limit direction
    if zero_for_one {
        if sqrt_price_limit >= current_sqrt_price_x96 || sqrt_price_limit <= MIN_SQRT_RATIO {
            panic!("Invalid price limit");
        }
    } else {
        if sqrt_price_limit <= current_sqrt_price_x96 || sqrt_price_limit >= MAX_SQRT_RATIO {
            panic!("Invalid price limit");
        }
    }

    sqrt_price_limit
}

/// Compute the target sqrt price for a swap step (pure)
/// Returns the price to aim for in this step, clamped by limit
pub fn compute_step_target_price(
    sqrt_price_next_tick_x96: u128,
    sqrt_price_limit_x96: u128,
    zero_for_one: bool,
) -> u128 {
    if zero_for_one {
        if sqrt_price_next_tick_x96 < sqrt_price_limit_x96 {
            sqrt_price_limit_x96
        } else {
            sqrt_price_next_tick_x96
        }
    } else {
        if sqrt_price_next_tick_x96 > sqrt_price_limit_x96 {
            sqrt_price_limit_x96
        } else {
            sqrt_price_next_tick_x96
        }
    }
}

/// Update swap state amounts after a step (pure)
/// Returns updated (amount_remaining, amount_calculated)
pub fn update_swap_amounts(
    amount_remaining: i128,
    amount_calculated: i128,
    step_amount_in: u128,
    step_amount_out: u128,
    step_fee_amount: u128,
    exact_input: bool,
) -> (i128, i128) {
    if exact_input {
        (
            amount_remaining - (step_amount_in + step_fee_amount) as i128,
            amount_calculated - step_amount_out as i128,
        )
    } else {
        (
            amount_remaining + step_amount_out as i128,
            amount_calculated + (step_amount_in + step_fee_amount) as i128,
        )
    }
}

/// Compute fee growth delta (pure)
/// Returns the fee growth increment for this step
pub fn compute_fee_growth_delta(fee_amount: u128, liquidity: u128) -> u128 {
    if liquidity > 0 {
        (fee_amount as u128) << 128 / liquidity
    } else {
        0
    }
}

/// Determine the next tick after a price change (pure)
/// Returns (new_tick, should_cross_tick, tick_to_cross)
pub fn compute_tick_transition(
    sqrt_price_x96: u128,
    sqrt_price_next_tick_x96: u128,
    tick_next: i32,
    zero_for_one: bool,
    tick_initialized: bool,
) -> (i32, bool) {
    if sqrt_price_x96 == sqrt_price_next_tick_x96 {
        // We reached the next tick
        let new_tick = if zero_for_one {
            tick_next - 1
        } else {
            tick_next
        };
        (new_tick, tick_initialized)
    } else {
        // Price changed but didn't reach next tick - need to recalculate tick
        // Note: actual tick calculation requires env, done in caller
        (tick_next, false)
    }
}

/// Compute final amounts from swap state (pure)
pub fn compute_final_amounts(
    amount_specified: i128,
    amount_remaining: i128,
    amount_calculated: i128,
    zero_for_one: bool,
    exact_input: bool,
) -> (i128, i128) {
    if zero_for_one == exact_input {
        (amount_specified - amount_remaining, amount_calculated)
    } else {
        (amount_calculated, amount_specified - amount_remaining)
    }
}

/// Initialize swap state from pool state (pure)
pub fn init_swap_state(
    amount_specified: i128,
    sqrt_price_x96: u128,
    tick: i32,
    liquidity: u128,
    fee_growth_global_x128: u128,
) -> SwapState {
    SwapState {
        amount_remaining: amount_specified,
        amount_calculated: 0,
        sqrt_price_x96,
        tick,
        liquidity,
        fee_growth_global_x128,
    }
}

/// Check if swap loop should continue (pure)
pub fn should_continue_swap(
    amount_remaining: i128,
    sqrt_price_x96: u128,
    sqrt_price_limit: u128,
    tick_crossings: u32,
) -> bool {
    amount_remaining != 0
        && sqrt_price_x96 != sqrt_price_limit
        && tick_crossings < MAX_TICK_CROSSINGS_PER_SWAP
}

// ============================================================================
// SIDE EFFECT FUNCTIONS - Storage and token operations
// ============================================================================

/// Apply computed swap result to pool state (side effect)
fn apply_swap_to_state(
    env: &Env,
    computation: &SwapComputation,
    fee_growth_global_0_x128: u128,
    fee_growth_global_1_x128: u128,
) {
    let mut state = get_state(env);

    state.sqrt_price_x96 = computation.sqrt_price_x96;
    state.tick = computation.tick;
    state.liquidity = computation.liquidity;

    if computation.fee_growth_is_token0 {
        state.fee_growth_global_0_x128 = computation.fee_growth_global_x128;
        state.fee_growth_global_1_x128 = fee_growth_global_1_x128;
    } else {
        state.fee_growth_global_0_x128 = fee_growth_global_0_x128;
        state.fee_growth_global_1_x128 = computation.fee_growth_global_x128;
    }

    set_state(env, &state);
}

/// Transfer tokens for a swap (side effect)
fn transfer_swap_tokens(
    env: &Env,
    token0: &Address,
    token1: &Address,
    recipient: &Address,
    amount0: i128,
    amount1: i128,
    zero_for_one: bool,
) {
    let contract_address = env.current_contract_address();
    let token0_client = token::Client::new(env, token0);
    let token1_client = token::Client::new(env, token1);

    if zero_for_one {
        // User pays token0, receives token1
        if amount0 > 0 {
            token0_client.transfer(recipient, &contract_address, &amount0);
        }
        if amount1 < 0 {
            token1_client.transfer(&contract_address, recipient, &(-amount1));
        }
    } else {
        // User pays token1, receives token0
        if amount1 > 0 {
            token1_client.transfer(recipient, &contract_address, &amount1);
        }
        if amount0 < 0 {
            token0_client.transfer(&contract_address, recipient, &(-amount0));
        }
    }
}

// ============================================================================
// MAIN ENTRY POINT - Orchestrates pure computation and side effects
// ============================================================================

/// Execute a swap
///
/// This function orchestrates the swap by:
/// 1. Validating parameters (pure)
/// 2. Computing swap steps in a loop (mixed - needs tick lookups)
/// 3. Applying state changes (side effect)
/// 4. Transferring tokens (side effect)
///
/// Note: The swap loop requires storage reads for tick bitmap lookups,
/// so it cannot be fully pure. However, individual steps use pure functions
/// that can be formally verified in isolation.
pub fn execute_swap(
    env: &Env,
    recipient: Address,
    zero_for_one: bool,
    amount_specified: i128,
    sqrt_price_limit_x96: u128,
) -> (i128, i128) {
    let config = get_config(env);
    let state = get_state(env);

    // === PHASE 1: Validation (pure) ===
    let sqrt_price_limit = validate_swap_params(
        amount_specified,
        zero_for_one,
        sqrt_price_limit_x96,
        state.sqrt_price_x96,
    );

    let exact_input = amount_specified > 0;

    // === PHASE 2: Initialize swap state ===
    let initial_fee_growth = if zero_for_one {
        state.fee_growth_global_0_x128
    } else {
        state.fee_growth_global_1_x128
    };

    let mut swap_state = init_swap_state(
        amount_specified,
        state.sqrt_price_x96,
        state.tick,
        state.liquidity,
        initial_fee_growth,
    );

    let mut tick_crossings: u32 = 0;

    // === PHASE 3: Swap loop ===
    // Note: This loop requires storage reads for tick bitmap, so it's not fully pure.
    // However, each step uses pure helper functions that can be verified.
    while should_continue_swap(
        swap_state.amount_remaining,
        swap_state.sqrt_price_x96,
        sqrt_price_limit,
        tick_crossings,
    ) {
        // Find next initialized tick (requires storage read)
        let (tick_next, initialized) = next_initialized_tick_within_one_word(
            env,
            swap_state.tick,
            config.tick_spacing,
            zero_for_one,
        );

        // Clamp to min/max tick (pure)
        let tick_next = tick_next.clamp(dex_types::MIN_TICK, dex_types::MAX_TICK);

        // Get sqrt price at next tick (pure computation with env for U256)
        let sqrt_price_next_x96 = get_sqrt_ratio_at_tick(env, tick_next);

        // Compute target price for this step (pure)
        let sqrt_ratio_target_x96 =
            compute_step_target_price(sqrt_price_next_x96, sqrt_price_limit, zero_for_one);

        // Compute swap step (pure computation with env for U256)
        let step = compute_swap_step(
            env,
            swap_state.sqrt_price_x96,
            sqrt_ratio_target_x96,
            swap_state.liquidity,
            swap_state.amount_remaining,
            config.fee,
        );

        // Update amounts (pure)
        let (new_amount_remaining, new_amount_calculated) = update_swap_amounts(
            swap_state.amount_remaining,
            swap_state.amount_calculated,
            step.amount_in,
            step.amount_out,
            step.fee_amount,
            exact_input,
        );
        swap_state.amount_remaining = new_amount_remaining;
        swap_state.amount_calculated = new_amount_calculated;

        // Update fee growth (pure)
        let fee_growth_delta = compute_fee_growth_delta(step.fee_amount, swap_state.liquidity);
        swap_state.fee_growth_global_x128 += fee_growth_delta;

        // Update sqrt price
        swap_state.sqrt_price_x96 = step.sqrt_ratio_next_x96;

        // Handle tick transition (pure logic, but crossing requires storage)
        let (new_tick, should_cross) = compute_tick_transition(
            swap_state.sqrt_price_x96,
            sqrt_price_next_x96,
            tick_next,
            zero_for_one,
            initialized,
        );

        if should_cross {
            // Cross tick (side effect - updates tick storage)
            let liquidity_net = cross(
                env,
                tick_next,
                state.fee_growth_global_0_x128,
                state.fee_growth_global_1_x128,
            );

            // Apply liquidity change (pure computation)
            let liquidity_delta = if zero_for_one {
                -liquidity_net
            } else {
                liquidity_net
            };
            swap_state.liquidity = add_delta(swap_state.liquidity, liquidity_delta);

            tick_crossings += 1;
        }

        // Update tick
        if swap_state.sqrt_price_x96 == sqrt_price_next_x96 {
            swap_state.tick = new_tick;
        } else if swap_state.sqrt_price_x96 != state.sqrt_price_x96 {
            // Price changed but didn't reach next tick
            swap_state.tick = dex_math::get_tick_at_sqrt_ratio(env, swap_state.sqrt_price_x96);
        }
    }

    // === PHASE 4: Compute final amounts (pure) ===
    let (amount0, amount1) = compute_final_amounts(
        amount_specified,
        swap_state.amount_remaining,
        swap_state.amount_calculated,
        zero_for_one,
        exact_input,
    );

    // === PHASE 5: Build computation result ===
    let computation = SwapComputation {
        amount0,
        amount1,
        sqrt_price_x96: swap_state.sqrt_price_x96,
        tick: swap_state.tick,
        liquidity: swap_state.liquidity,
        fee_growth_global_x128: swap_state.fee_growth_global_x128,
        fee_growth_is_token0: zero_for_one,
        ticks_crossed: tick_crossings,
    };

    // === PHASE 6: Apply state changes (side effect) ===
    apply_swap_to_state(
        env,
        &computation,
        state.fee_growth_global_0_x128,
        state.fee_growth_global_1_x128,
    );

    // === PHASE 7: Transfer tokens (side effect) ===
    transfer_swap_tokens(
        env,
        &config.token0,
        &config.token1,
        &recipient,
        amount0,
        amount1,
        zero_for_one,
    );

    (amount0, amount1)
}

// ============================================================================
// TESTS FOR PURE FUNCTIONS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === validate_swap_params tests ===

    #[test]
    fn test_validate_swap_params_exact_input_zero_for_one() {
        // Use a price in the valid range (between MIN and MAX)
        let current_price = dex_types::Q96; // Price = 1
        let limit = validate_swap_params(100, true, 0, current_price);
        assert_eq!(limit, MIN_SQRT_RATIO + 1);
    }

    #[test]
    fn test_validate_swap_params_exact_input_one_for_zero() {
        // Use a price in the valid range
        let current_price = dex_types::Q96; // Price = 1
        let limit = validate_swap_params(100, false, 0, current_price);
        assert_eq!(limit, MAX_SQRT_RATIO - 1);
    }

    #[test]
    fn test_validate_swap_params_with_explicit_limit() {
        // Use a price in the valid range, with explicit limit below it
        let current_price = dex_types::Q96; // Price = 1
        let explicit_limit = dex_types::Q96 / 2; // Below current price
        let limit = validate_swap_params(100, true, explicit_limit, current_price);
        assert_eq!(limit, explicit_limit);
    }

    #[test]
    #[should_panic(expected = "Amount must be non-zero")]
    fn test_validate_swap_params_zero_amount() {
        validate_swap_params(0, true, 0, 1000000);
    }

    #[test]
    #[should_panic(expected = "Invalid price limit")]
    fn test_validate_swap_params_invalid_limit_zero_for_one() {
        // For zero_for_one, limit must be < current price
        let current_price = dex_types::Q96;
        // Limit above current price is invalid for zero_for_one
        validate_swap_params(100, true, current_price + 1, current_price);
    }

    #[test]
    #[should_panic(expected = "Invalid price limit")]
    fn test_validate_swap_params_invalid_limit_one_for_zero() {
        // For one_for_zero, limit must be > current price
        let current_price = dex_types::Q96;
        // Limit below current price is invalid for one_for_zero
        validate_swap_params(100, false, current_price - 1, current_price);
    }

    // === compute_step_target_price tests ===

    #[test]
    fn test_compute_step_target_price_zero_for_one_tick_before_limit() {
        // Next tick price is above limit, so use tick price
        let tick_price = 500u128;
        let limit = 400u128;
        let target = compute_step_target_price(tick_price, limit, true);
        assert_eq!(target, tick_price);
    }

    #[test]
    fn test_compute_step_target_price_zero_for_one_limit_before_tick() {
        // Next tick price is below limit, so use limit
        let tick_price = 300u128;
        let limit = 400u128;
        let target = compute_step_target_price(tick_price, limit, true);
        assert_eq!(target, limit);
    }

    #[test]
    fn test_compute_step_target_price_one_for_zero_tick_before_limit() {
        // Next tick price is below limit, so use tick price
        let tick_price = 500u128;
        let limit = 600u128;
        let target = compute_step_target_price(tick_price, limit, false);
        assert_eq!(target, tick_price);
    }

    #[test]
    fn test_compute_step_target_price_one_for_zero_limit_before_tick() {
        // Next tick price is above limit, so use limit
        let tick_price = 700u128;
        let limit = 600u128;
        let target = compute_step_target_price(tick_price, limit, false);
        assert_eq!(target, limit);
    }

    // === update_swap_amounts tests ===

    #[test]
    fn test_update_swap_amounts_exact_input() {
        let (remaining, calculated) = update_swap_amounts(
            1000,  // amount_remaining
            0,     // amount_calculated
            100,   // step_amount_in
            95,    // step_amount_out
            5,     // step_fee_amount
            true,  // exact_input
        );
        // remaining -= (amount_in + fee) = 1000 - 105 = 895
        assert_eq!(remaining, 895);
        // calculated -= amount_out = 0 - 95 = -95
        assert_eq!(calculated, -95);
    }

    #[test]
    fn test_update_swap_amounts_exact_output() {
        let (remaining, calculated) = update_swap_amounts(
            -1000, // amount_remaining (negative for exact output)
            0,     // amount_calculated
            100,   // step_amount_in
            95,    // step_amount_out
            5,     // step_fee_amount
            false, // exact_output
        );
        // remaining += amount_out = -1000 + 95 = -905
        assert_eq!(remaining, -905);
        // calculated += (amount_in + fee) = 0 + 105 = 105
        assert_eq!(calculated, 105);
    }

    // === compute_fee_growth_delta tests ===

    #[test]
    fn test_compute_fee_growth_delta_with_liquidity() {
        let delta = compute_fee_growth_delta(100, 1000);
        // (100 << 128) / 1000
        assert!(delta > 0);
    }

    #[test]
    fn test_compute_fee_growth_delta_zero_liquidity() {
        let delta = compute_fee_growth_delta(100, 0);
        assert_eq!(delta, 0);
    }

    // === compute_tick_transition tests ===

    #[test]
    fn test_compute_tick_transition_reached_tick_zero_for_one() {
        let (new_tick, should_cross) = compute_tick_transition(
            1000,  // sqrt_price_x96 (reached next tick)
            1000,  // sqrt_price_next_tick_x96
            100,   // tick_next
            true,  // zero_for_one
            true,  // tick_initialized
        );
        assert_eq!(new_tick, 99); // tick_next - 1 for zero_for_one
        assert!(should_cross);
    }

    #[test]
    fn test_compute_tick_transition_reached_tick_one_for_zero() {
        let (new_tick, should_cross) = compute_tick_transition(
            1000,  // sqrt_price_x96 (reached next tick)
            1000,  // sqrt_price_next_tick_x96
            100,   // tick_next
            false, // one_for_zero
            true,  // tick_initialized
        );
        assert_eq!(new_tick, 100); // tick_next for one_for_zero
        assert!(should_cross);
    }

    #[test]
    fn test_compute_tick_transition_not_reached() {
        let (new_tick, should_cross) = compute_tick_transition(
            999,   // sqrt_price_x96 (didn't reach next tick)
            1000,  // sqrt_price_next_tick_x96
            100,   // tick_next
            true,  // zero_for_one
            true,  // tick_initialized
        );
        // Tick needs to be recalculated from price
        assert!(!should_cross);
    }

    #[test]
    fn test_compute_tick_transition_uninitialized_tick() {
        let (_, should_cross) = compute_tick_transition(
            1000,  // sqrt_price_x96
            1000,  // sqrt_price_next_tick_x96
            100,   // tick_next
            true,  // zero_for_one
            false, // tick NOT initialized
        );
        assert!(!should_cross); // Don't cross uninitialized ticks
    }

    // === compute_final_amounts tests ===

    #[test]
    fn test_compute_final_amounts_exact_input_zero_for_one() {
        let (amount0, amount1) = compute_final_amounts(
            1000,  // amount_specified
            100,   // amount_remaining
            -900,  // amount_calculated
            true,  // zero_for_one
            true,  // exact_input
        );
        // zero_for_one && exact_input => same branch
        assert_eq!(amount0, 900);  // amount_specified - amount_remaining
        assert_eq!(amount1, -900); // amount_calculated
    }

    #[test]
    fn test_compute_final_amounts_exact_output_one_for_zero() {
        let (amount0, amount1) = compute_final_amounts(
            -1000, // amount_specified (negative for exact output)
            -100,  // amount_remaining
            900,   // amount_calculated
            false, // one_for_zero
            false, // exact_output
        );
        // !zero_for_one && !exact_input => same branch
        assert_eq!(amount0, -900); // amount_specified - amount_remaining
        assert_eq!(amount1, 900);  // amount_calculated
    }

    // === should_continue_swap tests ===

    #[test]
    fn test_should_continue_swap_has_remaining() {
        assert!(should_continue_swap(100, 1000, 500, 0));
    }

    #[test]
    fn test_should_continue_swap_no_remaining() {
        assert!(!should_continue_swap(0, 1000, 500, 0));
    }

    #[test]
    fn test_should_continue_swap_reached_limit() {
        assert!(!should_continue_swap(100, 500, 500, 0)); // price == limit
    }

    #[test]
    fn test_should_continue_swap_max_crossings() {
        assert!(!should_continue_swap(100, 1000, 500, MAX_TICK_CROSSINGS_PER_SWAP));
    }

    // === init_swap_state tests ===

    #[test]
    fn test_init_swap_state() {
        let state = init_swap_state(1000, 79228162514264337593543950336, 0, 50000, 0);
        assert_eq!(state.amount_remaining, 1000);
        assert_eq!(state.amount_calculated, 0);
        assert_eq!(state.sqrt_price_x96, 79228162514264337593543950336);
        assert_eq!(state.tick, 0);
        assert_eq!(state.liquidity, 50000);
        assert_eq!(state.fee_growth_global_x128, 0);
    }
}
