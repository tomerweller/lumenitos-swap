#![no_std]

use dex_math::{compute_swap_step, get_sqrt_ratio_at_tick};
use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, PoolState};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, Vec};

#[contract]
pub struct DexQuoter;

/// Storage keys
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Factory,
}

/// Quote result
#[contracttype]
#[derive(Clone)]
pub struct QuoteResult {
    pub amount_out: i128,
    pub sqrt_price_after_x96: u128,
    pub tick_after: i32,
}

#[contractimpl]
impl DexQuoter {
    /// Initialize quoter with factory address
    pub fn initialize(env: Env, factory: Address) {
        if env.storage().instance().has(&DataKey::Factory) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Factory, &factory);
    }

    /// Quote exact input single swap
    /// Returns the expected output amount without executing the swap
    pub fn quote_exact_input_single(
        env: Env,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: i128,
        sqrt_price_limit_x96: u128,
    ) -> QuoteResult {
        let factory = get_factory(&env);
        let pool = get_pool(&env, &factory, &token_in, &token_out, fee);

        let zero_for_one = token_in < token_out;
        let sqrt_price_limit = if sqrt_price_limit_x96 == 0 {
            if zero_for_one {
                MIN_SQRT_RATIO + 1
            } else {
                MAX_SQRT_RATIO - 1
            }
        } else {
            sqrt_price_limit_x96
        };

        // Get current pool state
        let state = get_pool_state(&env, &pool);
        let tick_spacing = get_pool_tick_spacing(&env, &pool);

        // Simulate swap
        simulate_swap(
            &env,
            &pool,
            state,
            tick_spacing,
            fee,
            zero_for_one,
            amount_in,
            sqrt_price_limit,
        )
    }

    /// Quote exact output single swap
    pub fn quote_exact_output_single(
        env: Env,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_out: i128,
        sqrt_price_limit_x96: u128,
    ) -> QuoteResult {
        let factory = get_factory(&env);
        let pool = get_pool(&env, &factory, &token_in, &token_out, fee);

        let zero_for_one = token_in < token_out;
        let sqrt_price_limit = if sqrt_price_limit_x96 == 0 {
            if zero_for_one {
                MIN_SQRT_RATIO + 1
            } else {
                MAX_SQRT_RATIO - 1
            }
        } else {
            sqrt_price_limit_x96
        };

        let state = get_pool_state(&env, &pool);
        let tick_spacing = get_pool_tick_spacing(&env, &pool);

        // Negative amount for exact output
        simulate_swap(
            &env,
            &pool,
            state,
            tick_spacing,
            fee,
            zero_for_one,
            -amount_out,
            sqrt_price_limit,
        )
    }

    /// Get factory address
    pub fn get_factory(env: Env) -> Address {
        get_factory(&env)
    }
}

fn get_factory(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Factory)
        .expect("Not initialized")
}

fn get_pool(env: &Env, factory: &Address, token_a: &Address, token_b: &Address, fee: u32) -> Address {
    let pool: Option<Address> = env.invoke_contract(
        factory,
        &Symbol::new(env, "get_pool"),
        (token_a, token_b, fee).into_val(env),
    );
    pool.expect("Pool not found")
}

fn get_pool_state(env: &Env, pool: &Address) -> PoolState {
    env.invoke_contract(pool, &Symbol::new(env, "get_state"), ().into_val(env))
}

fn get_pool_tick_spacing(env: &Env, pool: &Address) -> i32 {
    env.invoke_contract(pool, &Symbol::new(env, "tick_spacing"), ().into_val(env))
}

/// Simulate a swap without modifying state
fn simulate_swap(
    env: &Env,
    _pool: &Address,
    state: PoolState,
    tick_spacing: i32,
    fee: u32,
    zero_for_one: bool,
    amount_specified: i128,
    sqrt_price_limit_x96: u128,
) -> QuoteResult {
    let exact_input = amount_specified > 0;

    let mut amount_remaining = amount_specified;
    let mut amount_calculated: i128 = 0;
    let mut sqrt_price_x96 = state.sqrt_price_x96;
    let mut tick = state.tick;
    let mut liquidity = state.liquidity;

    // Simplified simulation - single step for now
    // Full implementation would iterate through ticks like the actual swap
    while amount_remaining != 0 && sqrt_price_x96 != sqrt_price_limit_x96 {
        // Get next tick (simplified - just use price limit as target)
        let sqrt_ratio_target_x96 = sqrt_price_limit_x96;

        // Compute swap step
        let step = compute_swap_step(
            env,
            sqrt_price_x96,
            sqrt_ratio_target_x96,
            liquidity,
            amount_remaining,
            fee,
        );

        // Update amounts
        if exact_input {
            amount_remaining -= (step.amount_in + step.fee_amount) as i128;
            amount_calculated -= step.amount_out as i128;
        } else {
            amount_remaining += step.amount_out as i128;
            amount_calculated += (step.amount_in + step.fee_amount) as i128;
        }

        sqrt_price_x96 = step.sqrt_ratio_next_x96;

        // Update tick
        if sqrt_price_x96 != state.sqrt_price_x96 {
            tick = dex_math::get_tick_at_sqrt_ratio(env, sqrt_price_x96);
        }

        // For simplicity, break after one step
        // Full implementation would continue through ticks
        break;
    }

    QuoteResult {
        amount_out: if exact_input {
            -amount_calculated
        } else {
            amount_calculated
        },
        sqrt_price_after_x96: sqrt_price_x96,
        tick_after: tick,
    }
}
