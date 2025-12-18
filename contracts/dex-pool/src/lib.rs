#![no_std]

mod invariants;
mod liquidity;
mod storage;
mod swap;
mod tick;

use dex_types::{PoolConfig, PoolState, PositionKey, PositionInfo, TickInfo};
use soroban_sdk::{contract, contractimpl, token, Address, Env};
use storage::{
    get_config, get_position, get_state, get_tick, set_config, set_position, set_state, set_tick,
    DataKey,
};

#[contract]
pub struct DexPool;

#[contractimpl]
impl DexPool {
    /// Initialize a new pool
    pub fn initialize(
        env: Env,
        factory: Address,
        token0: Address,
        token1: Address,
        fee: u32,
        tick_spacing: i32,
        sqrt_price_x96: u128,
    ) {
        // Ensure not already initialized
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        // Validate token ordering
        if token0 >= token1 {
            panic!("token0 must be less than token1");
        }

        // Calculate initial tick from sqrt price
        let tick = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_price_x96);

        // Store config
        let config = PoolConfig {
            factory,
            token0,
            token1,
            fee,
            tick_spacing,
            max_liquidity_per_tick: dex_types::max_liquidity_per_tick(tick_spacing),
        };
        set_config(&env, &config);

        // Store initial state
        let state = PoolState::new(sqrt_price_x96, tick);
        set_state(&env, &state);
    }

    /// Execute a swap
    ///
    /// # Arguments
    /// * `recipient` - Address to receive output tokens
    /// * `zero_for_one` - True if swapping token0 for token1
    /// * `amount_specified` - Positive for exact input, negative for exact output
    /// * `sqrt_price_limit_x96` - Price limit for the swap
    ///
    /// # Returns
    /// (amount0, amount1) - Negative values are amounts paid out
    pub fn swap(
        env: Env,
        recipient: Address,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: u128,
    ) -> (i128, i128) {
        swap::execute_swap(&env, recipient, zero_for_one, amount_specified, sqrt_price_limit_x96)
    }

    /// Add liquidity to a position
    ///
    /// # Returns
    /// (amount0, amount1) - Token amounts deposited
    pub fn mint(
        env: Env,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> (u128, u128) {
        recipient.require_auth();
        liquidity::mint(&env, recipient, tick_lower, tick_upper, amount)
    }

    /// Remove liquidity from a position
    ///
    /// # Returns
    /// (amount0, amount1) - Token amounts withdrawn
    pub fn burn(env: Env, tick_lower: i32, tick_upper: i32, amount: u128) -> (u128, u128) {
        liquidity::burn(&env, tick_lower, tick_upper, amount)
    }

    /// Collect accumulated fees from a position
    ///
    /// # Returns
    /// (amount0, amount1) - Fee amounts collected
    pub fn collect(
        env: Env,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount0_requested: u128,
        amount1_requested: u128,
    ) -> (u128, u128) {
        liquidity::collect(&env, recipient, tick_lower, tick_upper, amount0_requested, amount1_requested)
    }

    // === View Functions ===

    /// Get current pool state
    pub fn get_state(env: Env) -> PoolState {
        get_state(&env)
    }

    /// Get pool configuration
    pub fn get_config(env: Env) -> PoolConfig {
        get_config(&env)
    }

    /// Get tick info
    pub fn get_tick(env: Env, tick: i32) -> TickInfo {
        get_tick(&env, tick)
    }

    /// Get position info
    pub fn get_position(
        env: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
    ) -> PositionInfo {
        let key = PositionKey {
            owner,
            tick_lower,
            tick_upper,
        };
        get_position(&env, &key)
    }

    /// Get current sqrt price
    pub fn sqrt_price_x96(env: Env) -> u128 {
        get_state(&env).sqrt_price_x96
    }

    /// Get current tick
    pub fn tick(env: Env) -> i32 {
        get_state(&env).tick
    }

    /// Get current liquidity
    pub fn liquidity(env: Env) -> u128 {
        get_state(&env).liquidity
    }

    /// Get token0 address
    pub fn token0(env: Env) -> Address {
        get_config(&env).token0
    }

    /// Get token1 address
    pub fn token1(env: Env) -> Address {
        get_config(&env).token1
    }

    /// Get fee
    pub fn fee(env: Env) -> u32 {
        get_config(&env).fee
    }

    /// Get tick spacing
    pub fn tick_spacing(env: Env) -> i32 {
        get_config(&env).tick_spacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_types::Q96;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, Env};

    #[allow(dead_code)]
    fn setup_pool(env: &Env) -> (Address, Address, Address, Address) {
        let factory = Address::generate(env);
        let token0 = Address::generate(env);
        let token1 = Address::generate(env);

        // Ensure token0 < token1
        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());

        (t0, t1, factory, contract_id)
    }

    #[allow(dead_code)]
    fn init_pool(env: &Env, contract_id: &Address, factory: &Address, token0: &Address, token1: &Address) {
        let client = DexPoolClient::new(env, contract_id);
        client.initialize(factory, token0, token1, &3000u32, &60i32, &Q96);
    }

    // === Initialization Tests ===

    #[test]
    fn test_initialize_pool() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        // Initialize pool
        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);

        // Verify state
        let state = client.get_state();
        assert_eq!(state.sqrt_price_x96, Q96);
        assert_eq!(state.tick, 0);
        assert_eq!(state.liquidity, 0);

        // Verify config
        let config = client.get_config();
        assert_eq!(config.factory, factory);
        assert_eq!(config.token0, t0);
        assert_eq!(config.token1, t1);
        assert_eq!(config.fee, 3000);
        assert_eq!(config.tick_spacing, 60);
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);
        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);
    }

    #[test]
    #[should_panic(expected = "token0 must be less than token1")]
    fn test_initialize_wrong_token_order() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        // Pass tokens in wrong order
        client.initialize(&factory, &t1, &t0, &3000u32, &60i32, &Q96);
    }

    // === View Function Tests ===

    #[test]
    fn test_view_functions() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        let initial_price = Q96 * 2; // Price = 4 (sqrt = 2)
        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &initial_price);

        // Test individual view functions
        assert_eq!(client.sqrt_price_x96(), initial_price);
        assert!(client.tick() > 0); // Price > 1 means tick > 0
        assert_eq!(client.liquidity(), 0);
        assert_eq!(client.token0(), t0);
        assert_eq!(client.token1(), t1);
        assert_eq!(client.fee(), 3000);
        assert_eq!(client.tick_spacing(), 60);
    }

    #[test]
    fn test_get_tick_uninitialized() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);

        // Get uninitialized tick
        let tick_info = client.get_tick(&100);
        assert_eq!(tick_info.liquidity_gross, 0);
        assert_eq!(tick_info.liquidity_net, 0);
        assert!(!tick_info.initialized);
    }

    #[test]
    fn test_get_position_empty() {
        let env = Env::default();
        let factory = Address::generate(&env);
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_id = env.register(DexPool, ());
        let client = DexPoolClient::new(&env, &contract_id);

        client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);

        let owner = Address::generate(&env);
        let position = client.get_position(&owner, &-120, &120);
        assert_eq!(position.liquidity, 0);
        assert_eq!(position.tokens_owed_0, 0);
        assert_eq!(position.tokens_owed_1, 0);
    }

    // === Different Fee Tier Tests ===

    #[test]
    fn test_initialize_different_fee_tiers() {
        let env = Env::default();
        let factory = Address::generate(&env);

        // Test 0.05% fee tier (tick spacing 10)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &500u32, &10i32, &Q96);
            assert_eq!(client.fee(), 500);
            assert_eq!(client.tick_spacing(), 10);
        }

        // Test 0.3% fee tier (tick spacing 60)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);
            assert_eq!(client.fee(), 3000);
            assert_eq!(client.tick_spacing(), 60);
        }

        // Test 1% fee tier (tick spacing 200)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &10000u32, &200i32, &Q96);
            assert_eq!(client.fee(), 10000);
            assert_eq!(client.tick_spacing(), 200);
        }
    }

    // === Initial Price Tests ===

    #[test]
    fn test_initialize_with_different_prices() {
        let env = Env::default();
        let factory = Address::generate(&env);

        // Price = 1 (tick = 0)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &Q96);
            let tick = client.tick();
            assert!(tick.abs() <= 1, "Price 1 should give tick ~0");
        }

        // Price > 1 (tick > 0)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &(Q96 * 2));
            let tick = client.tick();
            assert!(tick > 0, "Price > 1 should give positive tick");
        }

        // Price < 1 (tick < 0)
        {
            let token0 = Address::generate(&env);
            let token1 = Address::generate(&env);
            let (t0, t1) = if token0 < token1 {
                (token0, token1)
            } else {
                (token1, token0)
            };
            let contract_id = env.register(DexPool, ());
            let client = DexPoolClient::new(&env, &contract_id);
            client.initialize(&factory, &t0, &t1, &3000u32, &60i32, &(Q96 / 2));
            let tick = client.tick();
            assert!(tick < 0, "Price < 1 should give negative tick");
        }
    }

    // === Max Liquidity Per Tick Tests ===

    #[test]
    fn test_max_liquidity_varies_by_tick_spacing() {
        let env = Env::default();
        let factory = Address::generate(&env);

        // Smaller tick spacing = more ticks = less liquidity per tick
        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);
        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_10 = env.register(DexPool, ());
        let client_10 = DexPoolClient::new(&env, &contract_10);
        client_10.initialize(&factory, &t0, &t1, &500u32, &10i32, &Q96);

        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);
        let (t0, t1) = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };

        let contract_200 = env.register(DexPool, ());
        let client_200 = DexPoolClient::new(&env, &contract_200);
        client_200.initialize(&factory, &t0, &t1, &10000u32, &200i32, &Q96);

        let config_10 = client_10.get_config();
        let config_200 = client_200.get_config();

        // Wider tick spacing should allow more liquidity per tick
        assert!(
            config_200.max_liquidity_per_tick > config_10.max_liquidity_per_tick,
            "Wider spacing should allow more liquidity per tick"
        );
    }
}
