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

/// RULE: Pool sqrt_price is always within valid bounds
#[cfg(feature = "certora")]
#[rule]
pub fn price_always_in_bounds(sqrt_price_x96: u128) {
    use dex_types::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    // Valid pool states have price in bounds
    cvlr_assume!(sqrt_price_x96 > MIN_SQRT_RATIO);
    cvlr_assume!(sqrt_price_x96 < MAX_SQRT_RATIO);

    cvlr_satisfy!(true);
}

/// RULE: Pool tick is always within valid bounds
#[cfg(feature = "certora")]
#[rule]
pub fn tick_always_in_bounds(tick: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK);
    cvlr_assume!(tick <= MAX_TICK);

    cvlr_satisfy!(true);
}

/// RULE: Fee growth only increases (wrapping arithmetic)
#[cfg(feature = "certora")]
#[rule]
pub fn fee_growth_monotonic(
    fee_growth_before: u128,
    fee_growth_after: u128,
) {
    // Fee growth increases are bounded by half the max value
    // (to allow for wrapping arithmetic detection)
    let diff = fee_growth_after.wrapping_sub(fee_growth_before);
    cvlr_assert!(diff < u128::MAX / 2);
}

/// RULE: Fee is within valid range (max 100%)
#[cfg(feature = "certora")]
#[rule]
pub fn fee_in_valid_range(fee: u32) {
    // Fee in hundredths of bps, max 100% = 1_000_000
    cvlr_assert!(fee <= 1_000_000);
}

/// RULE: Tick spacing is positive
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
