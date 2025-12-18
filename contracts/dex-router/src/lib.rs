#![no_std]

use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, Vec,
};

#[contract]
pub struct DexRouter;

/// Storage keys
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Factory,
}

/// Parameters for exact input single swap
#[contracttype]
#[derive(Clone)]
pub struct ExactInputSingleParams {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
    pub recipient: Address,
    pub deadline: u64,
    pub amount_in: i128,
    pub amount_out_minimum: i128,
    pub sqrt_price_limit_x96: u128,
}

/// Parameters for exact output single swap
#[contracttype]
#[derive(Clone)]
pub struct ExactOutputSingleParams {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
    pub recipient: Address,
    pub deadline: u64,
    pub amount_out: i128,
    pub amount_in_maximum: i128,
    pub sqrt_price_limit_x96: u128,
}

/// Path element for multi-hop swaps
#[contracttype]
#[derive(Clone)]
pub struct PathElement {
    pub token: Address,
    pub fee: u32,
}

#[contractimpl]
impl DexRouter {
    /// Initialize router with factory address
    pub fn initialize(env: Env, factory: Address) {
        if env.storage().instance().has(&DataKey::Factory) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Factory, &factory);
    }

    /// Swap exact input amount for maximum output (single pool)
    pub fn exact_input_single(env: Env, params: ExactInputSingleParams) -> i128 {
        params.recipient.require_auth();
        check_deadline(&env, params.deadline);

        let factory = get_factory(&env);
        let pool = get_pool(&env, &factory, &params.token_in, &params.token_out, params.fee);

        let zero_for_one = params.token_in < params.token_out;
        let sqrt_price_limit = if params.sqrt_price_limit_x96 == 0 {
            if zero_for_one {
                MIN_SQRT_RATIO + 1
            } else {
                MAX_SQRT_RATIO - 1
            }
        } else {
            params.sqrt_price_limit_x96
        };

        // Execute swap
        let (amount0, amount1) = invoke_swap(
            &env,
            &pool,
            &params.recipient,
            zero_for_one,
            params.amount_in,
            sqrt_price_limit,
        );

        let amount_out = if zero_for_one { -amount1 } else { -amount0 };

        if amount_out < params.amount_out_minimum {
            panic!("Insufficient output amount");
        }

        amount_out
    }

    /// Swap minimum input for exact output (single pool)
    pub fn exact_output_single(env: Env, params: ExactOutputSingleParams) -> i128 {
        params.recipient.require_auth();
        check_deadline(&env, params.deadline);

        let factory = get_factory(&env);
        let pool = get_pool(&env, &factory, &params.token_in, &params.token_out, params.fee);

        let zero_for_one = params.token_in < params.token_out;
        let sqrt_price_limit = if params.sqrt_price_limit_x96 == 0 {
            if zero_for_one {
                MIN_SQRT_RATIO + 1
            } else {
                MAX_SQRT_RATIO - 1
            }
        } else {
            params.sqrt_price_limit_x96
        };

        // Execute swap with negative amount (exact output)
        let (amount0, amount1) = invoke_swap(
            &env,
            &pool,
            &params.recipient,
            zero_for_one,
            -params.amount_out, // Negative for exact output
            sqrt_price_limit,
        );

        let amount_in = if zero_for_one { amount0 } else { amount1 };

        if amount_in > params.amount_in_maximum {
            panic!("Excessive input amount");
        }

        amount_in
    }

    /// Multi-hop exact input swap
    pub fn exact_input(
        env: Env,
        path: Vec<PathElement>,
        recipient: Address,
        deadline: u64,
        amount_in: i128,
        amount_out_minimum: i128,
    ) -> i128 {
        recipient.require_auth();
        check_deadline(&env, deadline);

        if path.len() < 2 {
            panic!("Invalid path length");
        }

        let factory = get_factory(&env);
        let mut current_amount = amount_in;

        // Execute swaps along path
        for i in 0..(path.len() - 1) {
            let token_in = path.get(i).unwrap().token.clone();
            let fee = path.get(i).unwrap().fee;
            let token_out = path.get(i + 1).unwrap().token.clone();

            let is_last = i == path.len() - 2;
            let swap_recipient = if is_last {
                recipient.clone()
            } else {
                env.current_contract_address()
            };

            let pool = get_pool(&env, &factory, &token_in, &token_out, fee);
            let zero_for_one = token_in < token_out;

            let sqrt_price_limit = if zero_for_one {
                MIN_SQRT_RATIO + 1
            } else {
                MAX_SQRT_RATIO - 1
            };

            let (amount0, amount1) = invoke_swap(
                &env,
                &pool,
                &swap_recipient,
                zero_for_one,
                current_amount,
                sqrt_price_limit,
            );

            current_amount = if zero_for_one { -amount1 } else { -amount0 };
        }

        if current_amount < amount_out_minimum {
            panic!("Insufficient output amount");
        }

        current_amount
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

fn check_deadline(env: &Env, deadline: u64) {
    let current_time = env.ledger().timestamp();
    if current_time > deadline {
        panic!("Transaction expired");
    }
}

fn get_pool(env: &Env, factory: &Address, token_a: &Address, token_b: &Address, fee: u32) -> Address {
    let pool: Option<Address> = env.invoke_contract(
        factory,
        &Symbol::new(env, "get_pool"),
        (token_a, token_b, fee).into_val(env),
    );
    pool.expect("Pool not found")
}

fn invoke_swap(
    env: &Env,
    pool: &Address,
    recipient: &Address,
    zero_for_one: bool,
    amount_specified: i128,
    sqrt_price_limit_x96: u128,
) -> (i128, i128) {
    env.invoke_contract(
        pool,
        &Symbol::new(env, "swap"),
        (recipient, zero_for_one, amount_specified, sqrt_price_limit_x96).into_val(env),
    )
}
