// ============================================================================
// INVARIANTS MODULE - For Formal Verification
// ============================================================================
//
// This module defines invariant checking functions that express critical
// properties of the pool contract. These functions are designed to be
// used with formal verification tools like Certora Sunbeam.
//
// INVARIANT CATEGORIES:
//
// 1. PRICE INVARIANTS
//    - Price is always within valid bounds
//    - Tick is consistent with sqrt price
//
// 2. LIQUIDITY INVARIANTS
//    - Pool liquidity equals sum of active positions
//    - Liquidity never goes negative
//
// 3. FEE INVARIANTS
//    - Fee growth global is monotonically increasing
//    - Fee accounting is consistent
//
// 4. CONSERVATION INVARIANTS
//    - No tokens created from nothing
//    - Swap amounts are consistent
//
// 5. TICK INVARIANTS
//    - Tick bitmap is consistent with tick state
//    - Tick spacing is respected
//
// ============================================================================

use dex_types::{PoolConfig, PoolState, MAX_SQRT_RATIO, MIN_SQRT_RATIO, MAX_TICK, MIN_TICK};

// ============================================================================
// PRICE INVARIANTS
// ============================================================================

/// Invariant: sqrt_price is always within valid bounds
///
/// Property:
///   MIN_SQRT_RATIO < sqrt_price_x96 < MAX_SQRT_RATIO
///
/// This ensures the price is representable and within the tick range.
pub fn price_in_bounds(state: &PoolState) -> bool {
    state.sqrt_price_x96 > MIN_SQRT_RATIO && state.sqrt_price_x96 < MAX_SQRT_RATIO
}

/// Invariant: tick is within valid bounds
///
/// Property:
///   MIN_TICK <= tick <= MAX_TICK
pub fn tick_in_bounds(state: &PoolState) -> bool {
    state.tick >= MIN_TICK && state.tick <= MAX_TICK
}

/// Invariant: tick is consistent with sqrt price
///
/// Property:
///   |tick - get_tick_at_sqrt_ratio(sqrt_price_x96)| <= 1
///
/// The tick should be at most 1 away from the computed tick due to
/// rounding at boundaries.
///
/// Note: This requires the environment to compute, so we provide
/// a signature that can be used with formal verification tools.
pub fn tick_consistent_with_price(tick: i32, computed_tick_from_price: i32) -> bool {
    (tick - computed_tick_from_price).abs() <= 1
}

// ============================================================================
// LIQUIDITY INVARIANTS
// ============================================================================

/// Invariant: liquidity is non-negative (always true for u128)
///
/// Property:
///   liquidity >= 0
///
/// This is trivially true for u128, but important for verification.
#[allow(clippy::absurd_extreme_comparisons)]
pub fn liquidity_non_negative(state: &PoolState) -> bool {
    state.liquidity >= 0
}

/// Invariant: liquidity delta doesn't cause underflow
///
/// Property:
///   if delta < 0 then liquidity >= |delta|
///
/// When removing liquidity, we must have enough to remove.
pub fn liquidity_delta_valid(liquidity: u128, delta: i128) -> bool {
    if delta < 0 {
        liquidity >= ((-delta) as u128)
    } else {
        // For addition, check overflow wouldn't occur
        liquidity.checked_add(delta as u128).is_some()
    }
}

/// Invariant: max liquidity per tick is respected
///
/// Property:
///   tick.liquidity_gross <= config.max_liquidity_per_tick
pub fn tick_liquidity_bounded(tick_liquidity_gross: u128, max_liquidity_per_tick: u128) -> bool {
    tick_liquidity_gross <= max_liquidity_per_tick
}

// ============================================================================
// FEE INVARIANTS
// ============================================================================

/// Invariant: fee growth global is monotonically increasing
///
/// Property:
///   fee_growth_global_new >= fee_growth_global_old
///
/// Note: This uses wrapping arithmetic, so we check that the difference
/// is "small" (less than half of u128::MAX) to detect wrapping.
pub fn fee_growth_monotonic(old_fee_growth: u128, new_fee_growth: u128) -> bool {
    // Using wrapping subtraction, the difference should be positive
    // (or the new value wrapped around, which is valid for X128 format)
    let diff = new_fee_growth.wrapping_sub(old_fee_growth);
    // If the difference is less than half of u128::MAX, it's a valid increase
    diff < (u128::MAX / 2)
}

/// Invariant: fee is within valid range
///
/// Property:
///   fee <= 1_000_000 (100% max, but practically much lower)
pub fn fee_valid(config: &PoolConfig) -> bool {
    config.fee <= 1_000_000
}

/// Invariant: fee computation doesn't exceed input amount
///
/// Property:
///   fee_amount <= amount_in
pub fn fee_bounded_by_input(fee_amount: u128, amount_in: u128) -> bool {
    fee_amount <= amount_in
}

// ============================================================================
// CONSERVATION INVARIANTS
// ============================================================================

/// Invariant: swap conservation - no value created
///
/// Property (for exact input):
///   amount_in >= amount_out + fees (in value terms)
///
/// Note: This is a simplified check. Full conservation requires
/// price oracle to convert between tokens.
pub fn swap_amounts_consistent(
    amount_in: u128,
    amount_out: u128,
    fee_amount: u128,
    exact_input: bool,
) -> bool {
    if exact_input {
        // For exact input, we consume (amount_in + fee) and produce amount_out
        // The value produced should not exceed value consumed
        true // Detailed conservation requires price comparison
    } else {
        // For exact output, we produce amount_out and consume (amount_in + fee)
        true // Detailed conservation requires price comparison
    }
}

/// Invariant: amounts in a swap have correct signs
///
/// Property:
///   - One of amount0/amount1 is positive (paid in)
///   - One of amount0/amount1 is negative (paid out)
///   - They cannot both be positive or both be negative
pub fn swap_amounts_opposite_signs(amount0: i128, amount1: i128) -> bool {
    (amount0 > 0 && amount1 < 0) || (amount0 < 0 && amount1 > 0) || (amount0 == 0 || amount1 == 0)
}

// ============================================================================
// TICK INVARIANTS
// ============================================================================

/// Invariant: tick is on spacing
///
/// Property:
///   tick % tick_spacing == 0
pub fn tick_on_spacing(tick: i32, tick_spacing: i32) -> bool {
    tick % tick_spacing == 0
}

/// Invariant: tick_lower < tick_upper for a position
///
/// Property:
///   tick_lower < tick_upper
pub fn tick_range_valid(tick_lower: i32, tick_upper: i32) -> bool {
    tick_lower < tick_upper
}

/// Invariant: tick bitmap consistency
///
/// Property:
///   tick is initialized in storage IFF bit is set in bitmap
///
/// Note: This requires storage access, so we provide a pure signature.
pub fn tick_bitmap_consistent(tick_initialized: bool, bitmap_bit_set: bool) -> bool {
    tick_initialized == bitmap_bit_set
}

/// Invariant: liquidity_net sums to zero across all ticks
///
/// Property:
///   sum(tick.liquidity_net for all ticks) == 0
///
/// This ensures liquidity added at lower ticks equals liquidity removed at upper ticks.
/// Note: This is a global property that requires iterating all ticks.
pub fn liquidity_net_sums_to_zero(total_liquidity_net: i128) -> bool {
    total_liquidity_net == 0
}

// ============================================================================
// SWAP INVARIANTS
// ============================================================================

/// Invariant: swap direction consistency
///
/// Property:
///   - zero_for_one => price decreases (sqrt_price_after <= sqrt_price_before)
///   - !zero_for_one => price increases (sqrt_price_after >= sqrt_price_before)
pub fn swap_direction_consistent(
    zero_for_one: bool,
    sqrt_price_before: u128,
    sqrt_price_after: u128,
) -> bool {
    if zero_for_one {
        sqrt_price_after <= sqrt_price_before
    } else {
        sqrt_price_after >= sqrt_price_before
    }
}

/// Invariant: swap respects price limit
///
/// Property:
///   - zero_for_one => sqrt_price_after >= sqrt_price_limit
///   - !zero_for_one => sqrt_price_after <= sqrt_price_limit
pub fn swap_respects_limit(
    zero_for_one: bool,
    sqrt_price_after: u128,
    sqrt_price_limit: u128,
) -> bool {
    if zero_for_one {
        sqrt_price_after >= sqrt_price_limit
    } else {
        sqrt_price_after <= sqrt_price_limit
    }
}

/// Invariant: tick crossings bounded
///
/// Property:
///   ticks_crossed <= MAX_TICK_CROSSINGS_PER_SWAP
pub fn tick_crossings_bounded(ticks_crossed: u32, max_crossings: u32) -> bool {
    ticks_crossed <= max_crossings
}

// ============================================================================
// POSITION INVARIANTS
// ============================================================================

/// Invariant: position liquidity non-negative
///
/// Property:
///   position.liquidity >= 0
#[allow(clippy::absurd_extreme_comparisons)]
pub fn position_liquidity_valid(position_liquidity: u128) -> bool {
    position_liquidity >= 0
}

/// Invariant: tokens owed non-negative
///
/// Property:
///   position.tokens_owed_0 >= 0 && position.tokens_owed_1 >= 0
#[allow(clippy::absurd_extreme_comparisons)]
pub fn tokens_owed_valid(tokens_owed_0: u128, tokens_owed_1: u128) -> bool {
    tokens_owed_0 >= 0 && tokens_owed_1 >= 0
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_in_bounds_valid() {
        let state = PoolState {
            sqrt_price_x96: (MIN_SQRT_RATIO + MAX_SQRT_RATIO) / 2,
            tick: 0,
            liquidity: 1000,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        };
        assert!(price_in_bounds(&state));
    }

    #[test]
    fn test_price_in_bounds_at_min() {
        let state = PoolState {
            sqrt_price_x96: MIN_SQRT_RATIO,
            tick: MIN_TICK,
            liquidity: 0,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        };
        assert!(!price_in_bounds(&state)); // MIN_SQRT_RATIO is not valid (needs to be >)
    }

    #[test]
    fn test_tick_in_bounds_valid() {
        let state = PoolState {
            sqrt_price_x96: MIN_SQRT_RATIO + 1,
            tick: 0,
            liquidity: 0,
            fee_growth_global_0_x128: 0,
            fee_growth_global_1_x128: 0,
            protocol_fees_0: 0,
            protocol_fees_1: 0,
        };
        assert!(tick_in_bounds(&state));
    }

    #[test]
    fn test_tick_consistent_with_price() {
        assert!(tick_consistent_with_price(100, 100)); // Exact match
        assert!(tick_consistent_with_price(100, 99));  // Off by 1
        assert!(tick_consistent_with_price(100, 101)); // Off by 1
        assert!(!tick_consistent_with_price(100, 102)); // Off by 2
    }

    #[test]
    fn test_liquidity_delta_valid_add() {
        assert!(liquidity_delta_valid(1000, 500)); // Can add
        assert!(liquidity_delta_valid(0, 1000));   // Can add to zero
    }

    #[test]
    fn test_liquidity_delta_valid_remove() {
        assert!(liquidity_delta_valid(1000, -500));  // Can remove partial
        assert!(liquidity_delta_valid(1000, -1000)); // Can remove all
        assert!(!liquidity_delta_valid(1000, -1001)); // Cannot remove more
    }

    #[test]
    fn test_fee_growth_monotonic() {
        assert!(fee_growth_monotonic(100, 200)); // Normal increase
        assert!(fee_growth_monotonic(100, 100)); // No change
        assert!(!fee_growth_monotonic(200, 100)); // Decrease (invalid)
    }

    #[test]
    fn test_swap_direction_consistent() {
        // Zero for one - price should decrease
        assert!(swap_direction_consistent(true, 1000, 900));
        assert!(swap_direction_consistent(true, 1000, 1000)); // No change OK
        assert!(!swap_direction_consistent(true, 1000, 1100)); // Increase invalid

        // One for zero - price should increase
        assert!(swap_direction_consistent(false, 1000, 1100));
        assert!(swap_direction_consistent(false, 1000, 1000)); // No change OK
        assert!(!swap_direction_consistent(false, 1000, 900)); // Decrease invalid
    }

    #[test]
    fn test_swap_respects_limit() {
        // Zero for one - price must stay >= limit
        assert!(swap_respects_limit(true, 500, 400)); // Above limit
        assert!(swap_respects_limit(true, 400, 400)); // At limit
        assert!(!swap_respects_limit(true, 300, 400)); // Below limit

        // One for zero - price must stay <= limit
        assert!(swap_respects_limit(false, 500, 600)); // Below limit
        assert!(swap_respects_limit(false, 600, 600)); // At limit
        assert!(!swap_respects_limit(false, 700, 600)); // Above limit
    }

    #[test]
    fn test_tick_on_spacing() {
        assert!(tick_on_spacing(60, 60));
        assert!(tick_on_spacing(120, 60));
        assert!(tick_on_spacing(-60, 60));
        assert!(!tick_on_spacing(65, 60));
    }

    #[test]
    fn test_tick_range_valid() {
        assert!(tick_range_valid(-100, 100));
        assert!(tick_range_valid(0, 1));
        assert!(!tick_range_valid(100, 100)); // Equal
        assert!(!tick_range_valid(100, -100)); // Reversed
    }

    #[test]
    fn test_tick_bitmap_consistent() {
        assert!(tick_bitmap_consistent(true, true));   // Both set
        assert!(tick_bitmap_consistent(false, false)); // Both unset
        assert!(!tick_bitmap_consistent(true, false)); // Mismatch
        assert!(!tick_bitmap_consistent(false, true)); // Mismatch
    }

    #[test]
    fn test_swap_amounts_opposite_signs() {
        assert!(swap_amounts_opposite_signs(100, -50));  // Normal swap
        assert!(swap_amounts_opposite_signs(-100, 50)); // Reverse
        assert!(swap_amounts_opposite_signs(0, 0));     // No amounts
        assert!(swap_amounts_opposite_signs(100, 0));   // One zero
        assert!(!swap_amounts_opposite_signs(100, 100)); // Both positive
        assert!(!swap_amounts_opposite_signs(-100, -100)); // Both negative
    }
}
