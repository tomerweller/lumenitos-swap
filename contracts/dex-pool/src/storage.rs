use dex_types::{PoolConfig, PoolState, PositionInfo, PositionKey, TickInfo};
use soroban_sdk::{contracttype, Env};

// ============================================================================
// SOROBAN RESOURCE LIMITS - Critical constraints for pool operations:
// ============================================================================
// - Ledger entry size: 128 KiB max per entry
// - Read entries per tx: 100 entries / 200 KB
// - Write entries per tx: 50 entries / 132 KB
// - Max footprint keys: 100 keys per tx
//
// Storage design considerations:
// - Each tick (~65 bytes) is stored as a separate entry
// - Each bitmap word (u128 = 16 bytes) is stored as a separate entry
// - Each position (~120 bytes) is stored as a separate entry
// - Empty entries are automatically removed to save storage
//
// Swap operation limits:
// - Each tick crossing requires 1 read + 1 write to tick data
// - Each bitmap word lookup requires 1 read (potentially 1 write)
// - Max ticks crossable per swap: ~40-45 (conservative estimate)
//   to stay within 50 write entry limit while leaving room for state updates
//
// Position operation limits:
// - Mint/burn touches 2 tick entries + 1 position + state
// - Well within the 50 write entry limit
// ============================================================================

/// Maximum number of tick crossings allowed per swap operation.
/// This prevents exceeding Soroban's write entry limit (50 entries).
/// Each tick crossing requires ~2 storage writes (tick data + potentially bitmap).
/// We reserve some writes for state updates.
pub const MAX_TICK_CROSSINGS_PER_SWAP: u32 = 40;

/// Storage keys for the pool contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Pool configuration (Instance storage)
    Config,
    /// Current pool state (Instance storage)
    State,
    /// Tick data: tick_index -> TickInfo (Persistent storage)
    Tick(i32),
    /// Tick bitmap: word_position -> u128 bitmap (Persistent storage)
    TickBitmap(i32),
    /// Position data: PositionKey -> PositionInfo (Persistent storage)
    Position(PositionKey),
}

// TTL constants
const INSTANCE_TTL_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_TTL_EXTEND: u32 = 518400; // ~30 days
const PERSISTENT_TTL_THRESHOLD: u32 = 17280;
const PERSISTENT_TTL_EXTEND: u32 = 518400;

/// Extend instance storage TTL
pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
}

/// Extend persistent storage TTL for a key
pub fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
}

// === Config ===

pub fn get_config(env: &Env) -> PoolConfig {
    extend_instance_ttl(env);
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .expect("Pool not initialized")
}

pub fn set_config(env: &Env, config: &PoolConfig) {
    env.storage().instance().set(&DataKey::Config, config);
    extend_instance_ttl(env);
}

// === State ===

pub fn get_state(env: &Env) -> PoolState {
    extend_instance_ttl(env);
    env.storage()
        .instance()
        .get(&DataKey::State)
        .expect("Pool not initialized")
}

pub fn set_state(env: &Env, state: &PoolState) {
    env.storage().instance().set(&DataKey::State, state);
    extend_instance_ttl(env);
}

// === Tick ===

pub fn get_tick(env: &Env, tick: i32) -> TickInfo {
    let key = DataKey::Tick(tick);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_default()
}

pub fn set_tick(env: &Env, tick: i32, info: &TickInfo) {
    let key = DataKey::Tick(tick);
    if info.liquidity_gross == 0 && !info.initialized {
        // Remove empty tick
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, info);
        extend_persistent_ttl(env, &key);
    }
}

pub fn has_tick(env: &Env, tick: i32) -> bool {
    let key = DataKey::Tick(tick);
    env.storage().persistent().has(&key)
}

// === Tick Bitmap ===

pub fn get_tick_bitmap_word(env: &Env, word_pos: i32) -> u128 {
    let key = DataKey::TickBitmap(word_pos);
    env.storage().persistent().get(&key).unwrap_or(0u128)
}

pub fn set_tick_bitmap_word(env: &Env, word_pos: i32, bitmap: u128) {
    let key = DataKey::TickBitmap(word_pos);
    if bitmap == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &bitmap);
        extend_persistent_ttl(env, &key);
    }
}

// === Position ===

pub fn get_position(env: &Env, key: &PositionKey) -> PositionInfo {
    let data_key = DataKey::Position(key.clone());
    env.storage()
        .persistent()
        .get(&data_key)
        .unwrap_or_default()
}

pub fn set_position(env: &Env, key: &PositionKey, info: &PositionInfo) {
    let data_key = DataKey::Position(key.clone());
    if info.liquidity == 0 && info.tokens_owed_0 == 0 && info.tokens_owed_1 == 0 {
        // Remove empty position
        env.storage().persistent().remove(&data_key);
    } else {
        env.storage().persistent().set(&data_key, info);
        extend_persistent_ttl(env, &data_key);
    }
}
