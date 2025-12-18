use crate::storage::{get_config, get_position, get_state, set_position, set_state};
use crate::tick::{flip_tick, get_fee_growth_inside, update as update_tick};
use dex_math::{add_delta, get_sqrt_ratio_at_tick};
use dex_types::{PositionInfo, PositionKey};
use soroban_sdk::{token, Address, Env};

/// Mint (add) liquidity to a position
pub fn mint(
    env: &Env,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    amount: u128,
) -> (u128, u128) {
    if amount == 0 {
        panic!("Amount must be non-zero");
    }

    let config = get_config(env);
    let mut state = get_state(env);

    // Validate ticks
    validate_ticks(tick_lower, tick_upper, config.tick_spacing);

    // Calculate amounts needed
    let sqrt_ratio_lower = get_sqrt_ratio_at_tick(env, tick_lower);
    let sqrt_ratio_upper = get_sqrt_ratio_at_tick(env, tick_upper);

    let (amount0, amount1) = dex_math::get_amounts_for_liquidity(
        env,
        state.sqrt_price_x96,
        sqrt_ratio_lower,
        sqrt_ratio_upper,
        amount,
    );

    // Update ticks
    let flipped_lower = update_tick(
        env,
        tick_lower,
        state.tick,
        amount as i128,
        state.fee_growth_global_0_x128,
        state.fee_growth_global_1_x128,
        false, // lower tick
        config.max_liquidity_per_tick,
    );

    let flipped_upper = update_tick(
        env,
        tick_upper,
        state.tick,
        amount as i128,
        state.fee_growth_global_0_x128,
        state.fee_growth_global_1_x128,
        true, // upper tick
        config.max_liquidity_per_tick,
    );

    // Update tick bitmap if ticks were flipped
    if flipped_lower {
        flip_tick(env, tick_lower, config.tick_spacing);
    }
    if flipped_upper {
        flip_tick(env, tick_upper, config.tick_spacing);
    }

    // Update position
    let position_key = PositionKey {
        owner: recipient.clone(),
        tick_lower,
        tick_upper,
    };

    let (fee_growth_inside_0, fee_growth_inside_1) = get_fee_growth_inside(
        env,
        tick_lower,
        tick_upper,
        state.tick,
        state.fee_growth_global_0_x128,
        state.fee_growth_global_1_x128,
    );

    update_position(
        env,
        &position_key,
        amount as i128,
        fee_growth_inside_0,
        fee_growth_inside_1,
    );

    // Update liquidity if position is in range
    if state.tick >= tick_lower && state.tick < tick_upper {
        state.liquidity = add_delta(state.liquidity, amount as i128);
        set_state(env, &state);
    }

    // Transfer tokens from user
    let contract_address = env.current_contract_address();

    if amount0 > 0 {
        let token0_client = token::Client::new(env, &config.token0);
        token0_client.transfer(&recipient, &contract_address, &(amount0 as i128));
    }

    if amount1 > 0 {
        let token1_client = token::Client::new(env, &config.token1);
        token1_client.transfer(&recipient, &contract_address, &(amount1 as i128));
    }

    (amount0, amount1)
}

/// Burn (remove) liquidity from a position
pub fn burn(env: &Env, tick_lower: i32, tick_upper: i32, amount: u128) -> (u128, u128) {
    let config = get_config(env);
    let mut state = get_state(env);

    // Get caller as position owner
    let owner = env.current_contract_address(); // TODO: Get actual caller

    // Validate ticks
    validate_ticks(tick_lower, tick_upper, config.tick_spacing);

    // Calculate amounts to return
    let sqrt_ratio_lower = get_sqrt_ratio_at_tick(env, tick_lower);
    let sqrt_ratio_upper = get_sqrt_ratio_at_tick(env, tick_upper);

    let (amount0, amount1) = dex_math::get_amounts_for_liquidity(
        env,
        state.sqrt_price_x96,
        sqrt_ratio_lower,
        sqrt_ratio_upper,
        amount,
    );

    if amount > 0 {
        // Update ticks (negative liquidity delta)
        let flipped_lower = update_tick(
            env,
            tick_lower,
            state.tick,
            -(amount as i128),
            state.fee_growth_global_0_x128,
            state.fee_growth_global_1_x128,
            false,
            config.max_liquidity_per_tick,
        );

        let flipped_upper = update_tick(
            env,
            tick_upper,
            state.tick,
            -(amount as i128),
            state.fee_growth_global_0_x128,
            state.fee_growth_global_1_x128,
            true,
            config.max_liquidity_per_tick,
        );

        // Update tick bitmap if ticks were flipped
        if flipped_lower {
            flip_tick(env, tick_lower, config.tick_spacing);
        }
        if flipped_upper {
            flip_tick(env, tick_upper, config.tick_spacing);
        }

        // Update liquidity if position is in range
        if state.tick >= tick_lower && state.tick < tick_upper {
            state.liquidity = add_delta(state.liquidity, -(amount as i128));
            set_state(env, &state);
        }
    }

    // Update position and accumulate owed tokens
    let position_key = PositionKey {
        owner,
        tick_lower,
        tick_upper,
    };

    let (fee_growth_inside_0, fee_growth_inside_1) = get_fee_growth_inside(
        env,
        tick_lower,
        tick_upper,
        state.tick,
        state.fee_growth_global_0_x128,
        state.fee_growth_global_1_x128,
    );

    update_position(
        env,
        &position_key,
        -(amount as i128),
        fee_growth_inside_0,
        fee_growth_inside_1,
    );

    // Add burned amounts to tokens owed
    let mut position = get_position(env, &position_key);
    position.tokens_owed_0 += amount0;
    position.tokens_owed_1 += amount1;
    set_position(env, &position_key, &position);

    (amount0, amount1)
}

/// Collect fees and withdrawn tokens from a position
pub fn collect(
    env: &Env,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    amount0_requested: u128,
    amount1_requested: u128,
) -> (u128, u128) {
    let config = get_config(env);

    // Get position owner (caller)
    let owner = env.current_contract_address(); // TODO: Get actual caller

    let position_key = PositionKey {
        owner,
        tick_lower,
        tick_upper,
    };

    let mut position = get_position(env, &position_key);

    // Calculate amounts to collect (capped by requested)
    let amount0 = amount0_requested.min(position.tokens_owed_0);
    let amount1 = amount1_requested.min(position.tokens_owed_1);

    // Update position
    position.tokens_owed_0 -= amount0;
    position.tokens_owed_1 -= amount1;
    set_position(env, &position_key, &position);

    // Transfer tokens to recipient
    if amount0 > 0 {
        let token0_client = token::Client::new(env, &config.token0);
        token0_client.transfer(
            &env.current_contract_address(),
            &recipient,
            &(amount0 as i128),
        );
    }

    if amount1 > 0 {
        let token1_client = token::Client::new(env, &config.token1);
        token1_client.transfer(
            &env.current_contract_address(),
            &recipient,
            &(amount1 as i128),
        );
    }

    (amount0, amount1)
}

/// Update a position with liquidity change and fee accumulation
fn update_position(
    env: &Env,
    key: &PositionKey,
    liquidity_delta: i128,
    fee_growth_inside_0_x128: u128,
    fee_growth_inside_1_x128: u128,
) {
    let mut position = get_position(env, key);

    // Calculate fees earned since last update
    if position.liquidity > 0 {
        // Use mul_div to compute (fee_delta * liquidity) / 2^128
        // fee_growth is in X128 format, so we need to divide by 2^128
        let fee_delta_0 = fee_growth_inside_0_x128.wrapping_sub(position.fee_growth_inside_0_last_x128);
        let fee_delta_1 = fee_growth_inside_1_x128.wrapping_sub(position.fee_growth_inside_1_last_x128);

        // Compute tokens_owed = (fee_delta * liquidity) >> 128
        // Since we can't shift u128 by 128, we use mul_div with Q128
        let q128: u128 = 1u128 << 64; // Use 2^64 as intermediate
        let tokens_owed_0 = dex_math::mul_div(
            env,
            dex_math::mul_div(env, fee_delta_0, position.liquidity, q128),
            1,
            q128,
        );
        let tokens_owed_1 = dex_math::mul_div(
            env,
            dex_math::mul_div(env, fee_delta_1, position.liquidity, q128),
            1,
            q128,
        );

        position.tokens_owed_0 += tokens_owed_0;
        position.tokens_owed_1 += tokens_owed_1;
    }

    // Update position liquidity
    position.liquidity = add_delta(position.liquidity, liquidity_delta);

    // Update fee growth checkpoints
    position.fee_growth_inside_0_last_x128 = fee_growth_inside_0_x128;
    position.fee_growth_inside_1_last_x128 = fee_growth_inside_1_x128;

    set_position(env, key, &position);
}

/// Validate tick parameters
fn validate_ticks(tick_lower: i32, tick_upper: i32, tick_spacing: i32) {
    if tick_lower >= tick_upper {
        panic!("tick_lower must be less than tick_upper");
    }
    if tick_lower < dex_types::MIN_TICK {
        panic!("tick_lower too low");
    }
    if tick_upper > dex_types::MAX_TICK {
        panic!("tick_upper too high");
    }
    if tick_lower % tick_spacing != 0 {
        panic!("tick_lower not on spacing");
    }
    if tick_upper % tick_spacing != 0 {
        panic!("tick_upper not on spacing");
    }
}
