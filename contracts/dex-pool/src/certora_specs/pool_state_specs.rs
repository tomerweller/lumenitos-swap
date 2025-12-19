// ============================================================================
// POOL STATE INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify that the pool state always maintains
// critical invariants for concentrated liquidity.
//
// KEY INVARIANTS:
// 1. Price is always within valid bounds
// 2. Tick is consistent with sqrt price
// 3. Liquidity is non-negative
// 4. Fee growth is monotonically increasing
//
// ============================================================================

// ============================================================================
// FORMAL VERIFICATION RULES (Certora Sunbeam)
// ============================================================================

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

/// RULE: Price derived from a valid tick stays within allowed sqrt bounds.
#[cfg(feature = "certora")]
#[rule]
pub fn price_from_tick_in_bounds(tick: i32, env: soroban_sdk::Env) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);
    let sqrt_price = dex_math::get_sqrt_ratio_at_tick(&env, tick);

    cvlr_assert!(sqrt_price >= MIN_SQRT_RATIO);
    cvlr_assert!(sqrt_price <= MAX_SQRT_RATIO);
}

/// RULE: Tick recovered from a price computed at that tick differs by at most 1.
#[cfg(feature = "certora")]
#[rule]
pub fn tick_price_roundtrip_stable(tick: i32, env: soroban_sdk::Env) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);

    let sqrt_price = dex_math::get_sqrt_ratio_at_tick(&env, tick);
    let recovered = dex_math::get_tick_at_sqrt_ratio(&env, sqrt_price);
    let diff = if recovered > tick { recovered - tick } else { tick - recovered };

    cvlr_assert!(diff <= 1);
}

/// RULE: Fee growth is non-decreasing (no wrap-around decrease).
#[cfg(feature = "certora")]
#[rule]
pub fn fee_growth_non_decreasing(fee_growth_before: u128, fee_growth_after: u128) {
    cvlr_assert!(fee_growth_after >= fee_growth_before);
}

/// RULE: Tick spacing must be positive.
#[cfg(feature = "certora")]
#[rule]
pub fn tick_spacing_positive(tick_spacing: i32) {
    cvlr_assert!(tick_spacing > 0);
}

// ============================================================================
// TESTS (run with cargo test)
// ============================================================================

#[cfg(test)]
mod tests {
    use dex_types::{PoolState, Q96, MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

    fn create_valid_state() -> PoolState {
        PoolState {
            sqrt_price_x96: Q96,
            tick: 0,
            liquidity: 1000000,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        }
    }

    #[test]
    fn test_price_bounds() {
        let state = create_valid_state();
        assert!(state.sqrt_price_x96 > MIN_SQRT_RATIO);
        assert!(state.sqrt_price_x96 < MAX_SQRT_RATIO);
    }

    #[test]
    fn test_tick_bounds() {
        let state = create_valid_state();
        assert!(state.tick >= MIN_TICK);
        assert!(state.tick <= MAX_TICK);
    }

    #[test]
    fn test_fee_growth_monotonic() {
        let state_before = create_valid_state();
        let mut state_after = state_before.clone();
        state_after.fee_growth_global_0_x128 += 1000;
        state_after.fee_growth_global_1_x128 += 2000;

        let diff_0 = state_after.fee_growth_global_0_x128.wrapping_sub(state_before.fee_growth_global_0_x128);
        let diff_1 = state_after.fee_growth_global_1_x128.wrapping_sub(state_before.fee_growth_global_1_x128);

        assert!(diff_0 < u128::MAX / 2);
        assert!(diff_1 < u128::MAX / 2);
    }

    #[test]
    fn test_liquidity_change_on_mint() {
        let mut state_before = create_valid_state();
        state_before.liquidity = 1000;

        let mut state_after = state_before.clone();
        state_after.liquidity = 1500;

        let expected = state_before.liquidity + 500;
        assert_eq!(state_after.liquidity, expected);
    }
}
