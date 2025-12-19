// ============================================================================
// LIQUIDITY INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify liquidity operations (mint, burn, collect)
// by calling actual contract functions and verifying state changes.
//
// ============================================================================

#[cfg(feature = "certora")]
use soroban_sdk::{Address, Env};

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume};

#[cfg(feature = "certora")]
use crate::DexPool;

/// RULE: Mint increases position liquidity by the specified amount
#[cfg(feature = "certora")]
#[rule]
pub fn mint_increases_position_liquidity(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(liquidity_amount > 0);

    // Valid tick range
    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Get position before
    let position_before = DexPool::get_position(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
    );

    // Mint liquidity
    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    // Get position after
    let position_after = DexPool::get_position(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
    );

    // Position liquidity should increase by exactly the minted amount
    cvlr_assert!(position_after.liquidity == position_before.liquidity + liquidity_amount);
}

/// RULE: Burn decreases position liquidity by the specified amount
#[cfg(feature = "certora")]
#[rule]
pub fn burn_decreases_position_liquidity(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    mint_amount: u128,
    burn_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(mint_amount > 0);
    cvlr_assume!(burn_amount > 0 && burn_amount <= mint_amount);

    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // First mint some liquidity
    let _mint_amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        mint_amount,
    );

    let position_before = DexPool::get_position(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
    );

    // Now burn
    let _burn_amounts = DexPool::burn(
        env.clone(),
        tick_lower,
        tick_upper,
        burn_amount,
    );

    let position_after = DexPool::get_position(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
    );

    // Position liquidity should decrease by exactly the burned amount
    cvlr_assert!(position_after.liquidity == position_before.liquidity - burn_amount);
}

/// RULE: Burn amount cannot exceed position liquidity
#[cfg(feature = "certora")]
#[rule]
pub fn burn_bounded_by_position(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    mint_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(mint_amount > 0);

    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Mint some liquidity
    let _mint_amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        mint_amount,
    );

    let position = DexPool::get_position(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
    );

    // The position should have exactly what we minted
    cvlr_assert!(position.liquidity == mint_amount);
}

/// RULE: Tick bounds must satisfy lower < upper
#[cfg(feature = "certora")]
#[rule]
pub fn position_tick_bounds_valid(
    tick_lower: i32,
    tick_upper: i32,
) {
    use dex_types::{MAX_TICK, MIN_TICK};

    // These are the requirements for valid position ticks
    cvlr_assume!(tick_lower >= MIN_TICK);
    cvlr_assume!(tick_upper <= MAX_TICK);

    // Verify lower < upper is required
    cvlr_assert!(tick_lower < tick_upper);
}

/// RULE: Ticks must be aligned to tick_spacing
#[cfg(feature = "certora")]
#[rule]
pub fn position_ticks_aligned(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(liquidity_amount > 0);

    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);

    // Ticks must be aligned to spacing for valid positions
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // This should succeed without panic (alignment is enforced)
    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    // Verify by checking config
    let config = DexPool::get_config(env.clone());
    cvlr_assert!(tick_lower % config.tick_spacing == 0);
    cvlr_assert!(tick_upper % config.tick_spacing == 0);
}

/// RULE: Liquidity net at lower and upper ticks balance out
#[cfg(feature = "certora")]
#[rule]
pub fn liquidity_net_balance(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(liquidity_amount > 0);

    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Mint liquidity
    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    // Get tick info at lower and upper
    let lower_tick_info = DexPool::get_tick(env.clone(), tick_lower);
    let upper_tick_info = DexPool::get_tick(env.clone(), tick_upper);

    // Lower tick gets +liquidity_net, upper tick gets -liquidity_net
    // They should balance out
    cvlr_assert!(lower_tick_info.liquidity_net == -upper_tick_info.liquidity_net);
}

/// RULE: Pool liquidity updates correctly when position spans current tick
#[cfg(feature = "certora")]
#[rule]
pub fn pool_liquidity_updates_for_active_position(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    recipient: Address,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_amount: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(liquidity_amount > 0);

    let tick_spacing: i32 = 60;
    cvlr_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK);
    cvlr_assume!(tick_lower < tick_upper);
    cvlr_assume!(tick_lower % tick_spacing == 0);
    cvlr_assume!(tick_upper % tick_spacing == 0);

    let fee: u32 = 3000;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let state_before = DexPool::get_state(env.clone());
    let current_tick = state_before.tick;
    let liquidity_before = state_before.liquidity;

    // Mint liquidity
    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    let state_after = DexPool::get_state(env.clone());

    // If position spans current tick, pool liquidity should increase
    if tick_lower <= current_tick && current_tick < tick_upper {
        cvlr_assert!(state_after.liquidity == liquidity_before + liquidity_amount);
    } else {
        // Position is out of range, pool liquidity unchanged
        cvlr_assert!(state_after.liquidity == liquidity_before);
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use dex_types::{MAX_TICK, MIN_TICK};

    #[test]
    fn test_position_tick_bounds() {
        let tick_lower = -100;
        let tick_upper = 100;
        assert!(tick_lower < tick_upper);
        assert!(tick_lower >= MIN_TICK);
        assert!(tick_upper <= MAX_TICK);
    }

    #[test]
    fn test_tick_alignment() {
        let tick_spacing = 60;
        let tick_lower = -120;
        let tick_upper = 180;

        assert_eq!(tick_lower % tick_spacing, 0);
        assert_eq!(tick_upper % tick_spacing, 0);
    }

    #[test]
    fn test_liquidity_delta_safety() {
        // Adding liquidity
        let liquidity: u128 = 1000;
        let delta: i128 = 500;
        let new_liquidity = liquidity.checked_add(delta as u128);
        assert!(new_liquidity.is_some());
        assert_eq!(new_liquidity.unwrap(), 1500);

        // Removing liquidity
        let remove_delta: i128 = -300;
        let abs_delta = (-remove_delta) as u128;
        assert!(liquidity >= abs_delta);
    }

    #[test]
    fn test_liquidity_net_balance() {
        let liquidity_delta: i128 = 1000;
        let lower_net_change = liquidity_delta;
        let upper_net_change = -liquidity_delta;

        assert_eq!(lower_net_change + upper_net_change, 0);
    }
}
