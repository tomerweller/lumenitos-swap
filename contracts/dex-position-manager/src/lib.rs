#![no_std]

use dex_types::PositionData;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, Vec};

#[contract]
pub struct DexPositionManager;

/// Storage keys
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Factory address
    Factory,
    /// Next position ID counter
    NextPositionId,
    /// Position ID -> PositionData
    Position(u32),
    /// Position ID -> Owner
    PositionOwner(u32),
    /// Owner -> position count (indexed storage for scalability)
    OwnerPositionCount(Address),
    /// Owner -> index -> position ID (indexed storage to avoid unbounded Vec)
    OwnerPositionAt(Address, u32),
    /// Position ID -> index in owner's list (for O(1) removal)
    PositionIndex(u32),
    /// Position ID -> approved address
    Approval(u32),
    /// Owner -> operator -> approved for all
    ApprovalForAll(Address, Address),
}

// ============================================================================
// SOROBAN RESOURCE LIMITS - Important constraints:
// ============================================================================
// - Ledger entry size: 128 KiB max
// - Read entries per tx: 100 entries / 200 KB
// - Write entries per tx: 50 entries / 132 KB
//
// Design choices:
// - Owner positions use indexed storage (count + indexed entries)
//   instead of Vec to avoid unbounded single entry
// - Each position ID stored separately (~8 bytes each)
// - Removal uses swap-and-pop for O(1) operations
// - Pagination provided for querying positions
// ============================================================================

/// Mint parameters
#[contracttype]
#[derive(Clone)]
pub struct MintParams {
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub amount0_desired: i128,
    pub amount1_desired: i128,
    pub amount0_min: i128,
    pub amount1_min: i128,
    pub recipient: Address,
    pub deadline: u64,
}

/// Increase liquidity parameters
#[contracttype]
#[derive(Clone)]
pub struct IncreaseLiquidityParams {
    pub position_id: u32,
    pub amount0_desired: i128,
    pub amount1_desired: i128,
    pub amount0_min: i128,
    pub amount1_min: i128,
    pub deadline: u64,
}

/// Decrease liquidity parameters
#[contracttype]
#[derive(Clone)]
pub struct DecreaseLiquidityParams {
    pub position_id: u32,
    pub liquidity: u128,
    pub amount0_min: i128,
    pub amount1_min: i128,
    pub deadline: u64,
}

/// Collect parameters
#[contracttype]
#[derive(Clone)]
pub struct CollectParams {
    pub position_id: u32,
    pub recipient: Address,
    pub amount0_max: u128,
    pub amount1_max: u128,
}

#[contractimpl]
impl DexPositionManager {
    /// Initialize with factory address
    pub fn initialize(env: Env, factory: Address) {
        if env.storage().instance().has(&DataKey::Factory) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Factory, &factory);
        env.storage().instance().set(&DataKey::NextPositionId, &1u32);
    }

    /// Create new position and mint NFT
    /// Returns: (position_id, liquidity, amount0, amount1)
    pub fn mint(env: Env, params: MintParams) -> (u32, u128, i128, i128) {
        params.recipient.require_auth();
        check_deadline(&env, params.deadline);

        let factory = get_factory(&env);

        // Get or create pool
        let pool = get_pool(&env, &factory, &params.token0, &params.token1, params.fee);

        // Calculate liquidity from desired amounts
        let pool_state = get_pool_state(&env, &pool);
        let sqrt_ratio_lower = dex_math::get_sqrt_ratio_at_tick(&env, params.tick_lower);
        let sqrt_ratio_upper = dex_math::get_sqrt_ratio_at_tick(&env, params.tick_upper);

        let liquidity = dex_math::get_liquidity_for_amounts(
            &env,
            pool_state.sqrt_price_x96,
            sqrt_ratio_lower,
            sqrt_ratio_upper,
            params.amount0_desired as u128,
            params.amount1_desired as u128,
        );

        // Mint liquidity in pool
        let (amount0, amount1) = invoke_pool_mint(
            &env,
            &pool,
            &env.current_contract_address(),
            params.tick_lower,
            params.tick_upper,
            liquidity,
        );

        // Check minimums
        if (amount0 as i128) < params.amount0_min || (amount1 as i128) < params.amount1_min {
            panic!("Slippage check failed");
        }

        // Create NFT position
        let position_id = get_next_position_id(&env);

        let position_data = PositionData {
            pool: pool.clone(),
            tick_lower: params.tick_lower,
            tick_upper: params.tick_upper,
            liquidity,
            fee_growth_inside_0_last_x128: 0,
            fee_growth_inside_1_last_x128: 0,
            tokens_owed_0: 0,
            tokens_owed_1: 0,
        };

        // Store position
        env.storage()
            .persistent()
            .set(&DataKey::Position(position_id), &position_data);
        env.storage()
            .persistent()
            .set(&DataKey::PositionOwner(position_id), &params.recipient);

        // Add to owner's positions
        add_position_to_owner(&env, &params.recipient, position_id);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "position_minted"),),
            (position_id, params.recipient.clone(), liquidity),
        );

        (position_id, liquidity, amount0 as i128, amount1 as i128)
    }

    /// Add liquidity to existing position
    pub fn increase_liquidity(
        env: Env,
        params: IncreaseLiquidityParams,
    ) -> (u128, i128, i128) {
        check_deadline(&env, params.deadline);

        let owner = get_position_owner(&env, params.position_id);
        owner.require_auth();

        let mut position = get_position(&env, params.position_id);

        // Calculate additional liquidity
        let pool_state = get_pool_state(&env, &position.pool);
        let sqrt_ratio_lower = dex_math::get_sqrt_ratio_at_tick(&env, position.tick_lower);
        let sqrt_ratio_upper = dex_math::get_sqrt_ratio_at_tick(&env, position.tick_upper);

        let liquidity = dex_math::get_liquidity_for_amounts(
            &env,
            pool_state.sqrt_price_x96,
            sqrt_ratio_lower,
            sqrt_ratio_upper,
            params.amount0_desired as u128,
            params.amount1_desired as u128,
        );

        // Mint in pool
        let (amount0, amount1) = invoke_pool_mint(
            &env,
            &position.pool,
            &env.current_contract_address(),
            position.tick_lower,
            position.tick_upper,
            liquidity,
        );

        // Check minimums
        if (amount0 as i128) < params.amount0_min || (amount1 as i128) < params.amount1_min {
            panic!("Slippage check failed");
        }

        // Update position
        position.liquidity += liquidity;
        env.storage()
            .persistent()
            .set(&DataKey::Position(params.position_id), &position);

        (liquidity, amount0 as i128, amount1 as i128)
    }

    /// Remove liquidity from position
    pub fn decrease_liquidity(env: Env, params: DecreaseLiquidityParams) -> (i128, i128) {
        check_deadline(&env, params.deadline);

        let owner = get_position_owner(&env, params.position_id);
        owner.require_auth();

        let mut position = get_position(&env, params.position_id);

        if params.liquidity > position.liquidity {
            panic!("Insufficient liquidity");
        }

        // Burn in pool
        let (amount0, amount1) = invoke_pool_burn(
            &env,
            &position.pool,
            position.tick_lower,
            position.tick_upper,
            params.liquidity,
        );

        // Check minimums
        if (amount0 as i128) < params.amount0_min || (amount1 as i128) < params.amount1_min {
            panic!("Slippage check failed");
        }

        // Update position
        position.liquidity -= params.liquidity;
        position.tokens_owed_0 += amount0;
        position.tokens_owed_1 += amount1;
        env.storage()
            .persistent()
            .set(&DataKey::Position(params.position_id), &position);

        (amount0 as i128, amount1 as i128)
    }

    /// Collect fees and tokens from position
    pub fn collect(env: Env, params: CollectParams) -> (u128, u128) {
        let owner = get_position_owner(&env, params.position_id);
        // Allow owner or approved
        if !is_approved_or_owner(&env, &owner, params.position_id) {
            panic!("Not authorized");
        }

        let mut position = get_position(&env, params.position_id);

        // Collect from pool
        let (collected0, collected1) = invoke_pool_collect(
            &env,
            &position.pool,
            &params.recipient,
            position.tick_lower,
            position.tick_upper,
            params.amount0_max,
            params.amount1_max,
        );

        // Update tokens owed
        position.tokens_owed_0 = position.tokens_owed_0.saturating_sub(collected0);
        position.tokens_owed_1 = position.tokens_owed_1.saturating_sub(collected1);
        env.storage()
            .persistent()
            .set(&DataKey::Position(params.position_id), &position);

        (collected0, collected1)
    }

    /// Burn position NFT (requires zero liquidity)
    pub fn burn(env: Env, position_id: u32) {
        let owner = get_position_owner(&env, position_id);
        owner.require_auth();

        let position = get_position(&env, position_id);

        if position.liquidity != 0 {
            panic!("Position has liquidity");
        }

        if position.tokens_owed_0 != 0 || position.tokens_owed_1 != 0 {
            panic!("Position has uncollected tokens");
        }

        // Remove position
        env.storage()
            .persistent()
            .remove(&DataKey::Position(position_id));
        env.storage()
            .persistent()
            .remove(&DataKey::PositionOwner(position_id));
        env.storage()
            .persistent()
            .remove(&DataKey::Approval(position_id));

        // Remove from owner's list
        remove_position_from_owner(&env, &owner, position_id);

        env.events().publish(
            (Symbol::new(&env, "position_burned"),),
            (position_id,),
        );
    }

    // === NFT-like Ownership Functions ===

    /// Transfer position ownership
    pub fn transfer_from(env: Env, from: Address, to: Address, position_id: u32) {
        let owner = get_position_owner(&env, position_id);

        if owner != from {
            panic!("Not owner");
        }

        if !is_approved_or_owner(&env, &from, position_id) {
            panic!("Not authorized");
        }

        from.require_auth();

        // Update owner
        env.storage()
            .persistent()
            .set(&DataKey::PositionOwner(position_id), &to);

        // Clear approval
        env.storage()
            .persistent()
            .remove(&DataKey::Approval(position_id));

        // Update owner lists
        remove_position_from_owner(&env, &from, position_id);
        add_position_to_owner(&env, &to, position_id);

        env.events().publish(
            (Symbol::new(&env, "transfer"),),
            (from, to, position_id),
        );
    }

    /// Approve address to manage position
    pub fn approve(env: Env, to: Address, position_id: u32) {
        let owner = get_position_owner(&env, position_id);
        owner.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::Approval(position_id), &to);

        env.events().publish(
            (Symbol::new(&env, "approval"),),
            (owner, to, position_id),
        );
    }

    /// Set operator approval for all positions
    pub fn set_approval_for_all(env: Env, operator: Address, approved: bool) {
        let caller = env.current_contract_address(); // TODO: get actual caller
        caller.require_auth();

        let key = DataKey::ApprovalForAll(caller.clone(), operator.clone());
        if approved {
            env.storage().persistent().set(&key, &true);
        } else {
            env.storage().persistent().remove(&key);
        }

        env.events().publish(
            (Symbol::new(&env, "approval_for_all"),),
            (caller, operator, approved),
        );
    }

    // === View Functions ===

    /// Get position details
    pub fn get_position(env: Env, position_id: u32) -> PositionData {
        get_position(&env, position_id)
    }

    /// Get position count for owner
    pub fn balance_of(env: Env, owner: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerPositionCount(owner))
            .unwrap_or(0)
    }

    /// Get position ID at index for owner
    pub fn position_of_owner_by_index(env: Env, owner: Address, index: u32) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerPositionAt(owner, index))
    }

    /// Get positions for owner with pagination
    /// Returns up to `limit` position IDs starting from `start_index`
    /// Maximum limit is 50 to stay within Soroban's read entry limits
    pub fn positions_of_paginated(env: Env, owner: Address, start_index: u32, limit: u32) -> Vec<u32> {
        // Cap limit to prevent exceeding read entry limits
        let safe_limit = if limit > 50 { 50 } else { limit };

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerPositionCount(owner.clone()))
            .unwrap_or(0);

        let mut positions: Vec<u32> = Vec::new(&env);

        let end_index = if start_index + safe_limit > count {
            count
        } else {
            start_index + safe_limit
        };

        for i in start_index..end_index {
            if let Some(pos_id) = env
                .storage()
                .persistent()
                .get(&DataKey::OwnerPositionAt(owner.clone(), i))
            {
                positions.push_back(pos_id);
            }
        }

        positions
    }

    /// Get all positions for owner (for backward compatibility)
    /// WARNING: May fail for users with many positions due to read limits.
    /// Use positions_of_paginated for production code.
    pub fn positions_of(env: Env, owner: Address) -> Vec<u32> {
        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerPositionCount(owner.clone()))
            .unwrap_or(0);

        // Limit to 50 to stay within read entry limits
        let safe_count = if count > 50 { 50 } else { count };

        let mut positions: Vec<u32> = Vec::new(&env);
        for i in 0..safe_count {
            if let Some(pos_id) = env
                .storage()
                .persistent()
                .get(&DataKey::OwnerPositionAt(owner.clone(), i))
            {
                positions.push_back(pos_id);
            }
        }
        positions
    }

    /// Get owner of position
    pub fn owner_of(env: Env, position_id: u32) -> Address {
        get_position_owner(&env, position_id)
    }

    /// Get approved address for position
    pub fn get_approved(env: Env, position_id: u32) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Approval(position_id))
    }

    /// Check if operator is approved for all
    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovalForAll(owner, operator))
            .unwrap_or(false)
    }

    /// Get total positions count
    pub fn total_supply(env: Env) -> u32 {
        let next_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextPositionId)
            .unwrap_or(1);
        next_id - 1
    }
}

// === Helper Functions ===

fn get_factory(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Factory)
        .expect("Not initialized")
}

fn check_deadline(env: &Env, deadline: u64) {
    if env.ledger().timestamp() > deadline {
        panic!("Transaction expired");
    }
}

fn get_next_position_id(env: &Env) -> u32 {
    let id: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextPositionId)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&DataKey::NextPositionId, &(id + 1));
    id
}

fn get_position(env: &Env, position_id: u32) -> PositionData {
    env.storage()
        .persistent()
        .get(&DataKey::Position(position_id))
        .expect("Position not found")
}

fn get_position_owner(env: &Env, position_id: u32) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::PositionOwner(position_id))
        .expect("Position not found")
}

fn is_approved_or_owner(env: &Env, owner: &Address, position_id: u32) -> bool {
    let caller = env.current_contract_address(); // TODO: get actual caller

    if caller == *owner {
        return true;
    }

    // Check specific approval
    if let Some(approved) = env
        .storage()
        .persistent()
        .get::<_, Address>(&DataKey::Approval(position_id))
    {
        if approved == caller {
            return true;
        }
    }

    // Check approval for all
    env.storage()
        .persistent()
        .get(&DataKey::ApprovalForAll(owner.clone(), caller))
        .unwrap_or(false)
}

/// Add position to owner's indexed list - O(1) operation
fn add_position_to_owner(env: &Env, owner: &Address, position_id: u32) {
    // Get current count
    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::OwnerPositionCount(owner.clone()))
        .unwrap_or(0);

    // Store position at next index
    env.storage()
        .persistent()
        .set(&DataKey::OwnerPositionAt(owner.clone(), count), &position_id);

    // Store the index for this position (for O(1) removal)
    env.storage()
        .persistent()
        .set(&DataKey::PositionIndex(position_id), &count);

    // Increment count
    env.storage()
        .persistent()
        .set(&DataKey::OwnerPositionCount(owner.clone()), &(count + 1));
}

/// Remove position from owner's indexed list using swap-and-pop - O(1) operation
fn remove_position_from_owner(env: &Env, owner: &Address, position_id: u32) {
    // Get current count
    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::OwnerPositionCount(owner.clone()))
        .unwrap_or(0);

    if count == 0 {
        return;
    }

    // Get the index of the position to remove
    let index_to_remove: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::PositionIndex(position_id))
        .unwrap_or(0);

    let last_index = count - 1;

    // If not the last element, swap with the last element
    if index_to_remove != last_index {
        // Get the last position ID
        let last_position_id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerPositionAt(owner.clone(), last_index))
            .unwrap_or(0);

        // Move last position to the removed slot
        env.storage().persistent().set(
            &DataKey::OwnerPositionAt(owner.clone(), index_to_remove),
            &last_position_id,
        );

        // Update the index of the moved position
        env.storage()
            .persistent()
            .set(&DataKey::PositionIndex(last_position_id), &index_to_remove);
    }

    // Remove the last slot
    env.storage()
        .persistent()
        .remove(&DataKey::OwnerPositionAt(owner.clone(), last_index));

    // Remove the index entry for the removed position
    env.storage()
        .persistent()
        .remove(&DataKey::PositionIndex(position_id));

    // Decrement count
    if count > 1 {
        env.storage()
            .persistent()
            .set(&DataKey::OwnerPositionCount(owner.clone()), &(count - 1));
    } else {
        // Remove count entry when empty
        env.storage()
            .persistent()
            .remove(&DataKey::OwnerPositionCount(owner.clone()));
    }
}

fn get_pool(env: &Env, factory: &Address, token0: &Address, token1: &Address, fee: u32) -> Address {
    let pool: Option<Address> = env.invoke_contract(
        factory,
        &Symbol::new(env, "get_pool"),
        (token0, token1, fee).into_val(env),
    );
    pool.expect("Pool not found")
}

fn get_pool_state(env: &Env, pool: &Address) -> dex_types::PoolState {
    env.invoke_contract(pool, &Symbol::new(env, "get_state"), ().into_val(env))
}

fn invoke_pool_mint(
    env: &Env,
    pool: &Address,
    recipient: &Address,
    tick_lower: i32,
    tick_upper: i32,
    amount: u128,
) -> (u128, u128) {
    env.invoke_contract(
        pool,
        &Symbol::new(env, "mint"),
        (recipient, tick_lower, tick_upper, amount).into_val(env),
    )
}

fn invoke_pool_burn(
    env: &Env,
    pool: &Address,
    tick_lower: i32,
    tick_upper: i32,
    amount: u128,
) -> (u128, u128) {
    env.invoke_contract(
        pool,
        &Symbol::new(env, "burn"),
        (tick_lower, tick_upper, amount).into_val(env),
    )
}

fn invoke_pool_collect(
    env: &Env,
    pool: &Address,
    recipient: &Address,
    tick_lower: i32,
    tick_upper: i32,
    amount0_max: u128,
    amount1_max: u128,
) -> (u128, u128) {
    env.invoke_contract(
        pool,
        &Symbol::new(env, "collect"),
        (recipient, tick_lower, tick_upper, amount0_max, amount1_max).into_val(env),
    )
}
