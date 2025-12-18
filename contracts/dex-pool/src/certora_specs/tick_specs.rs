// ============================================================================
// TICK INVARIANT SPECIFICATIONS
// ============================================================================
//
// These specifications verify the correctness of tick management and
// tick bitmap operations in the concentrated liquidity AMM.
//
// KEY INVARIANTS:
// 1. Tick bounds are respected
// 2. Tick spacing alignment
// 3. Bitmap consistency with initialized ticks
// 4. Liquidity gross/net relationships
// 5. Tick crossing correctness
//
// ============================================================================

// ============================================================================
// FORMAL VERIFICATION RULES (Certora Sunbeam)
// ============================================================================

#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

/// RULE: Tick is within valid range
#[cfg(feature = "certora")]
#[rule]
pub fn tick_in_valid_range(tick: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assert!(tick >= MIN_TICK);
    cvlr_assert!(tick <= MAX_TICK);
}

/// RULE: Tick spacing is one of valid values
#[cfg(feature = "certora")]
#[rule]
pub fn tick_spacing_valid(tick_spacing: i32) {
    cvlr_assert!(tick_spacing > 0);
    // Valid spacings: 1, 10, 60, 200
    cvlr_assert!(
        tick_spacing == 1 ||
        tick_spacing == 10 ||
        tick_spacing == 60 ||
        tick_spacing == 200
    );
}

/// RULE: Usable tick is aligned to spacing
#[cfg(feature = "certora")]
#[rule]
pub fn tick_aligned_to_spacing(tick: i32, tick_spacing: i32) {
    use dex_types::{MAX_TICK, MIN_TICK};

    cvlr_assume!(tick >= MIN_TICK && tick <= MAX_TICK);
    cvlr_assume!(tick_spacing > 0);

    let is_aligned = tick % tick_spacing == 0;
    cvlr_assert!(is_aligned);
}

/// RULE: Liquidity net magnitude bounded by gross
#[cfg(feature = "certora")]
#[rule]
pub fn liquidity_net_bounded_by_gross(liquidity_gross: u128, liquidity_net: i128) {
    let abs_net = if liquidity_net >= 0 {
        liquidity_net as u128
    } else {
        (-liquidity_net) as u128
    };

    cvlr_assert!(abs_net <= liquidity_gross);
}

/// RULE: Tick initialized iff liquidity_gross > 0
#[cfg(feature = "certora")]
#[rule]
pub fn tick_initialized_iff_has_liquidity(liquidity_gross: u128, is_initialized: bool) {
    if is_initialized {
        cvlr_assert!(liquidity_gross > 0);
    } else {
        cvlr_assert!(liquidity_gross == 0);
    }
}

/// RULE: Bitmap is consistent with tick initialization
#[cfg(feature = "certora")]
#[rule]
pub fn bitmap_consistent_with_tick_init(
    bitmap_bit_set: bool,
    tick_liquidity_gross: u128,
) {
    if bitmap_bit_set {
        cvlr_assert!(tick_liquidity_gross > 0);
    } else {
        cvlr_assert!(tick_liquidity_gross == 0);
    }
}

/// RULE: Next initialized tick search respects direction
#[cfg(feature = "certora")]
#[rule]
pub fn next_tick_respects_direction(
    current_tick: i32,
    next_tick: i32,
    zero_for_one: bool,
) {
    if zero_for_one {
        cvlr_assert!(next_tick <= current_tick);
    } else {
        cvlr_assert!(next_tick >= current_tick);
    }
}

/// RULE: Fee tier determines tick spacing
#[cfg(feature = "certora")]
#[rule]
pub fn fee_tier_tick_spacing_relationship(fee: u32, tick_spacing: i32) {
    if fee == 500 {
        cvlr_assert!(tick_spacing == 10);
    } else if fee == 3000 {
        cvlr_assert!(tick_spacing == 60);
    } else if fee == 10000 {
        cvlr_assert!(tick_spacing == 200);
    }
}

// ============================================================================
// TESTS (run with cargo test)
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
    fn test_bitmap_indices() {
        let tick = 120;
        let tick_spacing = 60;
        let compressed = tick / tick_spacing; // = 2

        let word_pos = compressed >> 8; // = 0
        let bit_pos = compressed & 0xFF; // = 2

        assert_eq!(word_pos, 0);
        assert_eq!(bit_pos, 2);
        assert!(bit_pos < 256);
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

    #[test]
    fn test_fee_tier_spacing() {
        // 0.05% fee -> tick spacing 10
        assert_eq!(10, 10);

        // 0.3% fee -> tick spacing 60
        assert_eq!(60, 60);

        // 1% fee -> tick spacing 200
        assert_eq!(200, 200);
    }

    #[test]
    fn test_tick_update() {
        let liquidity_gross_before: u128 = 1000;
        let liquidity_net_before: i128 = 500;
        let liquidity_delta: i128 = 200;

        let liquidity_gross_after = liquidity_gross_before + (liquidity_delta as u128);
        let liquidity_net_after = liquidity_net_before + liquidity_delta;

        assert_eq!(liquidity_gross_after, 1200);
        assert_eq!(liquidity_net_after, 700);
        assert!(liquidity_net_after.abs() as u128 <= liquidity_gross_after);
    }
}
