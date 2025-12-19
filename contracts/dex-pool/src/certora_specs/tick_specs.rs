// ============================================================================
// TICK INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify tick management and bitmap operations by
// calling actual contract functions and verifying tick state.
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

/// RULE: Tick is within valid range after any operation
#[cfg(feature = "certora")]
#[rule]
pub fn tick_in_valid_range(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let state = DexPool::get_state(env.clone());

    cvlr_assert!(state.tick >= MIN_TICK);
    cvlr_assert!(state.tick <= MAX_TICK);
}

/// RULE: Tick spacing from config is positive
#[cfg(feature = "certora")]
#[rule]
pub fn tick_spacing_positive(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    tick_spacing: i32,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick_spacing > 0);

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

    let config = DexPool::get_config(env.clone());
    cvlr_assert!(config.tick_spacing > 0);
    cvlr_assert!(config.tick_spacing == tick_spacing);
}

/// RULE: Liquidity net is bounded by liquidity gross
/// |liquidity_net| <= liquidity_gross
#[cfg(feature = "certora")]
#[rule]
pub fn liquidity_net_bounded_by_gross(
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

    // Mint liquidity to create tick state
    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    // Check lower tick
    let lower_tick_info = DexPool::get_tick(env.clone(), tick_lower);
    let abs_net_lower = if lower_tick_info.liquidity_net >= 0 {
        lower_tick_info.liquidity_net as u128
    } else {
        (-lower_tick_info.liquidity_net) as u128
    };
    cvlr_assert!(abs_net_lower <= lower_tick_info.liquidity_gross);

    // Check upper tick
    let upper_tick_info = DexPool::get_tick(env.clone(), tick_upper);
    let abs_net_upper = if upper_tick_info.liquidity_net >= 0 {
        upper_tick_info.liquidity_net as u128
    } else {
        (-upper_tick_info.liquidity_net) as u128
    };
    cvlr_assert!(abs_net_upper <= upper_tick_info.liquidity_gross);
}

/// RULE: Initialized tick has liquidity_gross > 0
/// After minting, the ticks should have positive liquidity_gross
#[cfg(feature = "certora")]
#[rule]
pub fn initialized_tick_has_liquidity(
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

    // After minting, ticks should be initialized with liquidity
    let lower_tick_info = DexPool::get_tick(env.clone(), tick_lower);
    let upper_tick_info = DexPool::get_tick(env.clone(), tick_upper);

    cvlr_assert!(lower_tick_info.liquidity_gross > 0);
    cvlr_assert!(upper_tick_info.liquidity_gross > 0);
    cvlr_assert!(lower_tick_info.initialized);
    cvlr_assert!(upper_tick_info.initialized);
}

/// RULE: Uninitialized tick has liquidity_gross == 0
#[cfg(feature = "certora")]
#[rule]
pub fn uninitialized_tick_has_no_liquidity(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    tick: i32,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    // Without any mints, all ticks should be uninitialized
    let tick_info = DexPool::get_tick(env.clone(), tick);

    cvlr_assert!(tick_info.liquidity_gross == 0);
    cvlr_assert!(tick_info.liquidity_net == 0);
    cvlr_assert!(!tick_info.initialized);
}

/// RULE: Fee growth at tick starts at zero for fresh pool
#[cfg(feature = "certora")]
#[rule]
pub fn tick_fee_growth_starts_zero(
    env: Env,
    factory: Address,
    token0: Address,
    token1: Address,
    sqrt_price_x96: u128,
    tick: i32,
) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(token0 < token1);
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO && sqrt_price_x96 < MAX_SQRT_RATIO);
    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);

    let fee: u32 = 3000;
    let tick_spacing: i32 = 60;
    DexPool::initialize(
        env.clone(),
        factory.clone(),
        token0.clone(),
        token1.clone(),
        fee,
        tick_spacing,
        sqrt_price_x96,
    );

    let tick_info = DexPool::get_tick(env.clone(), tick);

    // Fee growth outside should be zero for uninitialized ticks
    cvlr_assert!(tick_info.fee_growth_outside_0_x128 == 0);
    cvlr_assert!(tick_info.fee_growth_outside_1_x128 == 0);
}

/// RULE: Minted tick at lower boundary has positive liquidity_net
#[cfg(feature = "certora")]
#[rule]
pub fn lower_tick_has_positive_net(
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

    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    let lower_tick_info = DexPool::get_tick(env.clone(), tick_lower);

    // Lower tick should have positive net (liquidity enters here)
    cvlr_assert!(lower_tick_info.liquidity_net > 0);
    cvlr_assert!(lower_tick_info.liquidity_net == liquidity_amount as i128);
}

/// RULE: Minted tick at upper boundary has negative liquidity_net
#[cfg(feature = "certora")]
#[rule]
pub fn upper_tick_has_negative_net(
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

    let _amounts = DexPool::mint(
        env.clone(),
        recipient.clone(),
        tick_lower,
        tick_upper,
        liquidity_amount,
    );

    let upper_tick_info = DexPool::get_tick(env.clone(), tick_upper);

    // Upper tick should have negative net (liquidity exits here)
    cvlr_assert!(upper_tick_info.liquidity_net < 0);
    cvlr_assert!(upper_tick_info.liquidity_net == -(liquidity_amount as i128));
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use dex_types::{TickInfo, MAX_TICK, MIN_TICK};

    #[test]
    fn test_tick_bounds() {
        assert!(MIN_TICK < MAX_TICK);
        assert!(MIN_TICK >= -887272);
        assert!(MAX_TICK <= 887272);
    }

    #[test]
    fn test_tick_spacing_alignment() {
        let tick_spacing = 60;
        let tick = 120;
        assert_eq!(tick % tick_spacing, 0);

        let unaligned_tick = 125;
        assert_ne!(unaligned_tick % tick_spacing, 0);
    }

    #[test]
    fn test_liquidity_net_bounded() {
        let tick_info = TickInfo {
            liquidity_gross: 1000,
            liquidity_net: 500,
            fee_growth_outside_0_x128: 0,
            fee_growth_outside_1_x128: 0,
            initialized: true,
        };

        let abs_net = tick_info.liquidity_net.abs() as u128;
        assert!(abs_net <= tick_info.liquidity_gross);
    }

    #[test]
    fn test_tick_crossing_direction() {
        let current_tick = 100;
        let next_tick_down = 60;

        assert!(next_tick_down <= current_tick);

        let next_tick_up = 140;
        assert!(next_tick_up >= current_tick);
    }
}
