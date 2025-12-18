#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, IntoVal, Symbol, Vec};

#[contract]
pub struct DexFactory;

/// Storage keys for Factory contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Pool WASM hash for deployment
    PoolWasmHash,
    /// Fee tier -> tick spacing mapping
    FeeTickSpacing(u32),
    /// (token0, token1, fee) -> pool address
    Pool(Address, Address, u32),
    /// Total number of pools created (counter for indexed storage)
    PoolCount,
    /// Pool address at index (indexed storage to avoid unbounded Vec)
    PoolAt(u32),
    /// Protocol fee recipient
    FeeRecipient,
    /// Protocol fee percentage (basis points)
    ProtocolFee,
}

// TTL constants
const INSTANCE_TTL_THRESHOLD: u32 = 17280;
const INSTANCE_TTL_EXTEND: u32 = 518400;
const PERSISTENT_TTL_THRESHOLD: u32 = 17280;
const PERSISTENT_TTL_EXTEND: u32 = 518400;

// ============================================================================
// SOROBAN RESOURCE LIMITS - Important constraints to be aware of:
// ============================================================================
// - Ledger entry size: 128 KiB max
// - Storage key size: 250 bytes max
// - Read entries per tx: 100 entries / 200 KB
// - Write entries per tx: 50 entries / 132 KB
// - Max footprint keys: 100 keys per tx
// - CPU instructions: 100M per tx
// - Memory: 40 MB per tx
//
// Design choices to stay within limits:
// - Pool list uses indexed storage (PoolCount + PoolAt) instead of Vec
//   to avoid a single unbounded ledger entry
// - Each pool address is stored separately (~56 bytes each)
// - Pagination is provided for querying pools
// ============================================================================

#[contractimpl]
impl DexFactory {
    /// Initialize factory with admin and pool WASM hash
    pub fn initialize(env: Env, admin: Address, pool_wasm_hash: BytesN<32>) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PoolWasmHash, &pool_wasm_hash);

        // Initialize default fee tiers
        env.storage()
            .instance()
            .set(&DataKey::FeeTickSpacing(500), &10i32); // 0.05%
        env.storage()
            .instance()
            .set(&DataKey::FeeTickSpacing(3000), &60i32); // 0.3%
        env.storage()
            .instance()
            .set(&DataKey::FeeTickSpacing(10000), &200i32); // 1%

        // Initialize pool counter (indexed storage instead of unbounded Vec)
        env.storage().instance().set(&DataKey::PoolCount, &0u32);

        extend_instance_ttl(&env);
    }

    /// Create a new pool for token pair with specified fee
    /// Returns the pool contract address
    pub fn create_pool(
        env: Env,
        token_a: Address,
        token_b: Address,
        fee: u32,
        initial_sqrt_price_x96: u128,
    ) -> Address {
        // Sort tokens
        let (token0, token1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        // Validate tokens are different
        if token0 == token1 {
            panic!("Identical tokens");
        }

        // Check pool doesn't already exist
        let pool_key = DataKey::Pool(token0.clone(), token1.clone(), fee);
        if env.storage().persistent().has(&pool_key) {
            panic!("Pool already exists");
        }

        // Get tick spacing for fee
        let tick_spacing = Self::get_fee_tick_spacing(env.clone(), fee);
        if tick_spacing == 0 {
            panic!("Fee not enabled");
        }

        // Get pool WASM hash
        let pool_wasm_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::PoolWasmHash)
            .expect("Not initialized");

        // Get current pool count for salt and indexing
        let pool_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PoolCount)
            .unwrap_or(0);

        // Create deterministic salt from pool count + fee
        let mut salt_bytes = [0u8; 32];
        salt_bytes[0..4].copy_from_slice(&pool_count.to_be_bytes());
        salt_bytes[4..8].copy_from_slice(&fee.to_be_bytes());
        let salt = BytesN::from_array(&env, &salt_bytes);

        // Deploy pool contract
        let pool_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(pool_wasm_hash, ());

        // Initialize the pool
        init_pool(
            &env,
            &pool_address,
            &env.current_contract_address(),
            &token0,
            &token1,
            &fee,
            &tick_spacing,
            &initial_sqrt_price_x96,
        );

        // Store pool address by token pair
        env.storage().persistent().set(&pool_key, &pool_address);
        extend_persistent_ttl(&env, &pool_key);

        // Store pool at index (indexed storage - O(1) append)
        let pool_at_key = DataKey::PoolAt(pool_count);
        env.storage()
            .persistent()
            .set(&pool_at_key, &pool_address);
        extend_persistent_ttl(&env, &pool_at_key);

        // Increment pool counter
        env.storage()
            .instance()
            .set(&DataKey::PoolCount, &(pool_count + 1));

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "pool_created"),),
            (token0, token1, fee, pool_address.clone()),
        );

        extend_instance_ttl(&env);
        pool_address
    }

    /// Get pool address for token pair and fee
    pub fn get_pool(env: Env, token_a: Address, token_b: Address, fee: u32) -> Option<Address> {
        let (token0, token1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        let pool_key = DataKey::Pool(token0, token1, fee);
        env.storage().persistent().get(&pool_key)
    }

    /// Enable a new fee tier
    pub fn enable_fee_amount(env: Env, fee: u32, tick_spacing: i32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        if tick_spacing <= 0 || tick_spacing > 16384 {
            panic!("Invalid tick spacing");
        }

        if fee >= 1_000_000 {
            panic!("Fee too high");
        }

        // Check not already set
        let key = DataKey::FeeTickSpacing(fee);
        if env.storage().instance().has(&key) {
            panic!("Fee already enabled");
        }

        env.storage().instance().set(&key, &tick_spacing);
        extend_instance_ttl(&env);
    }

    /// Get tick spacing for fee tier
    pub fn get_fee_tick_spacing(env: Env, fee: u32) -> i32 {
        extend_instance_ttl(&env);
        env.storage()
            .instance()
            .get(&DataKey::FeeTickSpacing(fee))
            .unwrap_or(0)
    }

    /// Set protocol fee recipient
    pub fn set_fee_recipient(env: Env, recipient: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::FeeRecipient, &recipient);
        extend_instance_ttl(&env);
    }

    /// Get protocol fee recipient
    pub fn get_fee_recipient(env: Env) -> Option<Address> {
        extend_instance_ttl(&env);
        env.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Get total number of pools created
    pub fn get_pool_count(env: Env) -> u32 {
        extend_instance_ttl(&env);
        env.storage()
            .instance()
            .get(&DataKey::PoolCount)
            .unwrap_or(0)
    }

    /// Get pool address at specific index
    pub fn get_pool_at(env: Env, index: u32) -> Option<Address> {
        let pool_at_key = DataKey::PoolAt(index);
        env.storage().persistent().get(&pool_at_key)
    }

    /// Get pools with pagination
    /// Returns up to `limit` pools starting from `start_index`
    /// Maximum limit is 50 to stay within Soroban's read entry limits
    pub fn get_pools_paginated(env: Env, start_index: u32, limit: u32) -> Vec<Address> {
        // Cap limit to prevent exceeding read entry limits (100 max, using 50 for safety)
        let safe_limit = if limit > 50 { 50 } else { limit };

        let pool_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PoolCount)
            .unwrap_or(0);

        let mut pools: Vec<Address> = Vec::new(&env);

        let end_index = if start_index + safe_limit > pool_count {
            pool_count
        } else {
            start_index + safe_limit
        };

        for i in start_index..end_index {
            if let Some(pool) = env.storage().persistent().get(&DataKey::PoolAt(i)) {
                pools.push_back(pool);
            }
        }

        pools
    }

    /// Get all deployed pools (for backward compatibility)
    /// WARNING: This may fail for large pool counts due to read limits.
    /// Use get_pools_paginated for production code.
    pub fn get_all_pools(env: Env) -> Vec<Address> {
        let pool_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PoolCount)
            .unwrap_or(0);

        // Limit to 50 pools to stay within read entry limits
        // For more pools, use get_pools_paginated
        let safe_count = if pool_count > 50 { 50 } else { pool_count };

        let mut pools: Vec<Address> = Vec::new(&env);
        for i in 0..safe_count {
            if let Some(pool) = env.storage().persistent().get(&DataKey::PoolAt(i)) {
                pools.push_back(pool);
            }
        }
        pools
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized")
    }

    /// Get pool WASM hash
    pub fn get_pool_wasm_hash(env: Env) -> BytesN<32> {
        extend_instance_ttl(&env);
        env.storage()
            .instance()
            .get(&DataKey::PoolWasmHash)
            .expect("Not initialized")
    }
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
}

fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
}

// Pool initialization via invoke
fn init_pool(
    env: &Env,
    pool_address: &Address,
    factory: &Address,
    token0: &Address,
    token1: &Address,
    fee: &u32,
    tick_spacing: &i32,
    sqrt_price_x96: &u128,
) {
    env.invoke_contract::<()>(
        pool_address,
        &Symbol::new(env, "initialize"),
        (factory, token0, token1, fee, tick_spacing, sqrt_price_x96).into_val(env),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, BytesN, Env};

    fn setup_factory(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(DexFactory, ());
        (admin, contract_id)
    }

    // === Initialization Tests ===

    #[test]
    fn test_initialize_factory() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Verify admin
        assert_eq!(client.get_admin(), admin);

        // Verify pool WASM hash
        assert_eq!(client.get_pool_wasm_hash(), pool_wasm_hash);

        // Verify default fee tiers
        assert_eq!(client.get_fee_tick_spacing(&500), 10); // 0.05%
        assert_eq!(client.get_fee_tick_spacing(&3000), 60); // 0.3%
        assert_eq!(client.get_fee_tick_spacing(&10000), 200); // 1%
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);
        client.initialize(&admin, &pool_wasm_hash);
    }

    // === Fee Tier Tests ===

    #[test]
    fn test_get_fee_tick_spacing_default() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Default fee tiers
        assert_eq!(client.get_fee_tick_spacing(&500), 10);
        assert_eq!(client.get_fee_tick_spacing(&3000), 60);
        assert_eq!(client.get_fee_tick_spacing(&10000), 200);

        // Unknown fee tier returns 0
        assert_eq!(client.get_fee_tick_spacing(&100), 0);
        assert_eq!(client.get_fee_tick_spacing(&5000), 0);
    }

    #[test]
    fn test_enable_fee_amount() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Enable a new fee tier
        client.enable_fee_amount(&100, &1); // 0.01% with tick spacing 1

        assert_eq!(client.get_fee_tick_spacing(&100), 1);
    }

    #[test]
    #[should_panic(expected = "Fee already enabled")]
    fn test_enable_existing_fee_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Try to enable existing fee tier
        client.enable_fee_amount(&500, &10);
    }

    #[test]
    #[should_panic(expected = "Invalid tick spacing")]
    fn test_enable_invalid_tick_spacing_zero() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        client.enable_fee_amount(&100, &0);
    }

    #[test]
    #[should_panic(expected = "Invalid tick spacing")]
    fn test_enable_invalid_tick_spacing_too_large() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        client.enable_fee_amount(&100, &20000); // > 16384
    }

    #[test]
    #[should_panic(expected = "Fee too high")]
    fn test_enable_fee_too_high() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        client.enable_fee_amount(&1_000_000, &100); // Fee >= 100%
    }

    // === Get Pool Tests ===

    #[test]
    fn test_get_pool_not_exists() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        let token0 = Address::generate(&env);
        let token1 = Address::generate(&env);

        // Pool should not exist
        let pool = client.get_pool(&token0, &token1, &3000);
        assert!(pool.is_none());
    }

    #[test]
    fn test_get_all_pools_empty() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        let pools = client.get_all_pools();
        assert_eq!(pools.len(), 0);

        // Also verify pool count
        assert_eq!(client.get_pool_count(), 0);
    }

    #[test]
    fn test_pool_count_and_pagination() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Initially pool count is 0
        assert_eq!(client.get_pool_count(), 0);

        // get_pool_at returns None for non-existent index
        assert!(client.get_pool_at(&0).is_none());

        // get_pools_paginated returns empty for no pools
        let paginated = client.get_pools_paginated(&0, &10);
        assert_eq!(paginated.len(), 0);
    }

    // === Fee Recipient Tests ===

    #[test]
    fn test_set_fee_recipient() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Initially no fee recipient
        let recipient = client.get_fee_recipient();
        assert!(recipient.is_none());

        // Set fee recipient
        let new_recipient = Address::generate(&env);
        client.set_fee_recipient(&new_recipient);

        let recipient = client.get_fee_recipient();
        assert_eq!(recipient, Some(new_recipient));
    }

    // === Admin Tests ===

    #[test]
    fn test_get_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        assert_eq!(client.get_admin(), admin);
    }

    // === Token Ordering Tests ===

    #[test]
    fn test_get_pool_token_order_invariant() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        // Querying with either order should return the same result
        let pool_ab = client.get_pool(&token_a, &token_b, &3000);
        let pool_ba = client.get_pool(&token_b, &token_a, &3000);

        assert_eq!(pool_ab, pool_ba);
    }

    // === Validation Tests ===

    #[test]
    fn test_fee_not_enabled_returns_zero() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Non-existent fee tier should return 0
        assert_eq!(client.get_fee_tick_spacing(&999), 0);
        assert_eq!(client.get_fee_tick_spacing(&50000), 0);
    }

    // === Edge Cases ===

    #[test]
    fn test_boundary_fee_values() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Enable minimum fee (0.0001%)
        client.enable_fee_amount(&1, &1);
        assert_eq!(client.get_fee_tick_spacing(&1), 1);

        // Enable near-maximum fee (99.9999%)
        client.enable_fee_amount(&999_999, &16384);
        assert_eq!(client.get_fee_tick_spacing(&999_999), 16384);
    }

    #[test]
    fn test_boundary_tick_spacing() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let contract_id = env.register(DexFactory, ());
        let client = DexFactoryClient::new(&env, &contract_id);

        let pool_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.initialize(&admin, &pool_wasm_hash);

        // Minimum valid tick spacing
        client.enable_fee_amount(&1, &1);
        assert_eq!(client.get_fee_tick_spacing(&1), 1);

        // Maximum valid tick spacing
        client.enable_fee_amount(&2, &16384);
        assert_eq!(client.get_fee_tick_spacing(&2), 16384);
    }
}
