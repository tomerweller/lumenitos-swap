use crate::storage::{get_config, get_state, set_state, MAX_TICK_CROSSINGS_PER_SWAP};
use crate::tick::{cross, next_initialized_tick_within_one_word};
use dex_math::{add_delta, compute_swap_step, get_sqrt_ratio_at_tick};
use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};
use soroban_sdk::{token, Address, Env};

/// Execute a swap
/// Note: This function limits the number of tick crossings per swap to stay
/// within Soroban's write entry limit (50 entries). If a swap would require
/// more tick crossings, it will be partially filled and the remaining amount
/// can be swapped in a subsequent transaction.
pub fn execute_swap(
    env: &Env,
    recipient: Address,
    zero_for_one: bool,
    amount_specified: i128,
    sqrt_price_limit_x96: u128,
) -> (i128, i128) {
    if amount_specified == 0 {
        panic!("Amount must be non-zero");
    }

    let config = get_config(env);
    let mut state = get_state(env);

    // Validate price limit
    let sqrt_price_limit = if sqrt_price_limit_x96 == 0 {
        if zero_for_one {
            MIN_SQRT_RATIO + 1
        } else {
            MAX_SQRT_RATIO - 1
        }
    } else {
        sqrt_price_limit_x96
    };

    if zero_for_one {
        if sqrt_price_limit >= state.sqrt_price_x96 || sqrt_price_limit <= MIN_SQRT_RATIO {
            panic!("Invalid price limit");
        }
    } else {
        if sqrt_price_limit <= state.sqrt_price_x96 || sqrt_price_limit >= MAX_SQRT_RATIO {
            panic!("Invalid price limit");
        }
    }

    let exact_input = amount_specified > 0;

    // Swap state
    let mut amount_remaining = amount_specified;
    let mut amount_calculated: i128 = 0;
    let mut sqrt_price_x96 = state.sqrt_price_x96;
    let mut tick = state.tick;
    let mut liquidity = state.liquidity;
    let mut fee_growth_global_x128 = if zero_for_one {
        state.fee_growth_global_0_x128
    } else {
        state.fee_growth_global_1_x128
    };

    // Track tick crossings to stay within Soroban's write entry limit
    let mut tick_crossings: u32 = 0;

    // Swap loop - stops if we run out of amount, hit price limit, or exceed tick crossing limit
    while amount_remaining != 0
        && sqrt_price_x96 != sqrt_price_limit
        && tick_crossings < MAX_TICK_CROSSINGS_PER_SWAP
    {
        // Find next initialized tick
        let (tick_next, initialized) =
            next_initialized_tick_within_one_word(env, tick, config.tick_spacing, zero_for_one);

        // Clamp to min/max tick
        let tick_next = tick_next.clamp(dex_types::MIN_TICK, dex_types::MAX_TICK);

        // Get sqrt price at next tick
        let sqrt_price_next_x96 = get_sqrt_ratio_at_tick(env, tick_next);

        // Determine target price for this step
        let sqrt_ratio_target_x96 = if zero_for_one {
            if sqrt_price_next_x96 < sqrt_price_limit {
                sqrt_price_limit
            } else {
                sqrt_price_next_x96
            }
        } else {
            if sqrt_price_next_x96 > sqrt_price_limit {
                sqrt_price_limit
            } else {
                sqrt_price_next_x96
            }
        };

        // Compute swap step
        let step = compute_swap_step(
            env,
            sqrt_price_x96,
            sqrt_ratio_target_x96,
            liquidity,
            amount_remaining,
            config.fee,
        );

        // Update amounts
        if exact_input {
            amount_remaining -= (step.amount_in + step.fee_amount) as i128;
            amount_calculated -= step.amount_out as i128;
        } else {
            amount_remaining += step.amount_out as i128;
            amount_calculated += (step.amount_in + step.fee_amount) as i128;
        }

        // Update fee growth
        if liquidity > 0 {
            let fee_growth_delta = (step.fee_amount as u128) << 128 / liquidity;
            fee_growth_global_x128 += fee_growth_delta;
        }

        // Update sqrt price
        sqrt_price_x96 = step.sqrt_ratio_next_x96;

        // Cross tick if we reached it
        if sqrt_price_x96 == sqrt_price_next_x96 {
            if initialized {
                let liquidity_net = cross(
                    env,
                    tick_next,
                    state.fee_growth_global_0_x128,
                    state.fee_growth_global_1_x128,
                );

                // Apply liquidity change
                let liquidity_net = if zero_for_one {
                    -liquidity_net
                } else {
                    liquidity_net
                };
                liquidity = add_delta(liquidity, liquidity_net);

                // Increment tick crossing counter
                tick_crossings += 1;
            }

            tick = if zero_for_one {
                tick_next - 1
            } else {
                tick_next
            };
        } else if sqrt_price_x96 != state.sqrt_price_x96 {
            // Price changed but didn't reach next tick
            tick = get_tick_at_sqrt_ratio(env, sqrt_price_x96);
        }
    }

    // Update state
    state.sqrt_price_x96 = sqrt_price_x96;
    state.tick = tick;
    state.liquidity = liquidity;

    if zero_for_one {
        state.fee_growth_global_0_x128 = fee_growth_global_x128;
    } else {
        state.fee_growth_global_1_x128 = fee_growth_global_x128;
    }

    set_state(env, &state);

    // Calculate final amounts
    let (amount0, amount1) = if zero_for_one == exact_input {
        (amount_specified - amount_remaining, amount_calculated)
    } else {
        (amount_calculated, amount_specified - amount_remaining)
    };

    // Transfer tokens
    let token0_client = token::Client::new(env, &config.token0);
    let token1_client = token::Client::new(env, &config.token1);
    let contract_address = env.current_contract_address();

    if zero_for_one {
        // User pays token0, receives token1
        if amount0 > 0 {
            token0_client.transfer(&recipient, &contract_address, &amount0);
        }
        if amount1 < 0 {
            token1_client.transfer(&contract_address, &recipient, &(-amount1));
        }
    } else {
        // User pays token1, receives token0
        if amount1 > 0 {
            token1_client.transfer(&recipient, &contract_address, &amount1);
        }
        if amount0 < 0 {
            token0_client.transfer(&contract_address, &recipient, &(-amount0));
        }
    }

    (amount0, amount1)
}

fn get_tick_at_sqrt_ratio(env: &Env, sqrt_price_x96: u128) -> i32 {
    dex_math::get_tick_at_sqrt_ratio(env, sqrt_price_x96)
}
