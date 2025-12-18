// ============================================================================
// TICK MODULE - Refactored for Formal Verification
// ============================================================================
//
// This module separates pure computation from side effects:
//
// 1. PURE BITMAP FUNCTIONS (formally verifiable):
//    - tick_to_bitmap_position: Convert tick to (word_pos, bit_pos)
//    - bitmap_position_to_tick: Convert (word_pos, bit) back to tick
//    - create_mask_at_or_below: Create mask for bits at or below position
//    - create_mask_at_or_above: Create mask for bits at or above position
//    - find_most_significant_bit: Find MSB in a word
//    - find_least_significant_bit: Find LSB in a word
//    - compute_next_tick_lte: Find next tick <= current (pure)
//    - compute_next_tick_gt: Find next tick > current (pure)
//
// 2. PURE TICK COMPUTATION FUNCTIONS:
//    - compute_liquidity_after_update: Calculate new liquidity values
//    - compute_fee_growth_after_cross: Calculate fee growth flip
//    - compute_fee_growth_inside: Calculate fee growth inside a range
//
// 3. SIDE EFFECT FUNCTIONS:
//    - update: Update tick in storage
//    - cross: Cross a tick (updates storage)
//    - flip_tick: Flip tick in bitmap (updates storage)
//    - next_initialized_tick_within_one_word: Find next tick (reads storage)
//
// ============================================================================

use crate::storage::{get_tick, get_tick_bitmap_word, set_tick, set_tick_bitmap_word};
use dex_types::TickInfo;
use soroban_sdk::Env;

// ============================================================================
// PURE BITMAP FUNCTIONS - No storage access, formally verifiable
// ============================================================================

/// Convert a tick index to bitmap position (pure)
/// Returns (word_position, bit_position)
///
/// # Properties (for formal verification)
/// - bit_position is always in [0, 127]
/// - bitmap_position_to_tick(tick_to_bitmap_position(t, s), s) == t (for aligned ticks)
pub fn tick_to_bitmap_position(tick: i32, tick_spacing: i32) -> (i32, u8) {
    let compressed = tick / tick_spacing;
    let word_pos = compressed >> 7; // divide by 128
    let bit_pos = (compressed.rem_euclid(128)) as u8;
    (word_pos, bit_pos)
}

/// Convert bitmap position back to tick (pure)
///
/// # Properties (for formal verification)
/// - Result is always aligned to tick_spacing
pub fn bitmap_position_to_tick(word_pos: i32, bit: i32, tick_spacing: i32) -> i32 {
    ((word_pos * 128) + bit) * tick_spacing
}

/// Create a mask for all bits at or below a given position (pure)
/// Used for searching left (decreasing tick values)
///
/// # Properties (for formal verification)
/// - bit_pos in [0, 127]
/// - Result has exactly (bit_pos + 1) bits set
/// - All set bits are in positions [0, bit_pos]
pub fn create_mask_at_or_below(bit_pos: u8) -> u128 {
    // Creates mask with bits 0 through bit_pos set
    // Example: bit_pos = 3 -> 0b1111 (bits 0,1,2,3)
    (1u128 << bit_pos) - 1 + (1u128 << bit_pos)
}

/// Create a mask for all bits at or above a given position (pure)
/// Used for searching right (increasing tick values)
///
/// # Properties (for formal verification)
/// - bit_pos in [0, 127]
/// - Result has bits set in positions [bit_pos, 127]
pub fn create_mask_at_or_above(bit_pos: u8) -> u128 {
    // Creates mask with bits bit_pos through 127 set
    // This is the complement of (bits 0 through bit_pos-1)
    !((1u128 << bit_pos) - 1)
}

/// Find the most significant bit (highest set bit) in a word (pure)
/// Returns None if word is 0
///
/// # Properties (for formal verification)
/// - Result in [0, 127] if Some
/// - If Some(b), then word & (1 << b) != 0
/// - If Some(b), then word & (mask for bits > b) == 0
pub fn find_most_significant_bit(word: u128) -> Option<u8> {
    if word == 0 {
        None
    } else {
        Some(127 - word.leading_zeros() as u8)
    }
}

/// Find the least significant bit (lowest set bit) in a word (pure)
/// Returns None if word is 0
///
/// # Properties (for formal verification)
/// - Result in [0, 127] if Some
/// - If Some(b), then word & (1 << b) != 0
/// - If Some(b), then word & (mask for bits < b) == 0
pub fn find_least_significant_bit(word: u128) -> Option<u8> {
    if word == 0 {
        None
    } else {
        Some(word.trailing_zeros() as u8)
    }
}

/// Compute the next initialized tick at or below current position (pure)
/// Returns (next_tick, is_initialized)
///
/// This is the pure computation given a bitmap word.
/// The actual storage lookup is done by the caller.
pub fn compute_next_tick_lte(
    word: u128,
    word_pos: i32,
    bit_pos: u8,
    tick_spacing: i32,
) -> (i32, bool) {
    let mask = create_mask_at_or_below(bit_pos);
    let masked = word & mask;

    match find_most_significant_bit(masked) {
        Some(msb) => {
            let tick = bitmap_position_to_tick(word_pos, msb as i32, tick_spacing);
            (tick, true)
        }
        None => {
            // No initialized tick in this word at or below current position
            // Return word boundary
            let tick = bitmap_position_to_tick(word_pos, 0, tick_spacing);
            (tick, false)
        }
    }
}

/// Compute the next initialized tick above current position (pure)
/// Returns (next_tick, is_initialized)
///
/// This is the pure computation given a bitmap word.
/// The actual storage lookup is done by the caller.
pub fn compute_next_tick_gt(
    word: u128,
    word_pos: i32,
    bit_pos: u8,
    tick_spacing: i32,
) -> (i32, bool) {
    let mask = create_mask_at_or_above(bit_pos);
    let masked = word & mask;

    match find_least_significant_bit(masked) {
        Some(lsb) => {
            let tick = bitmap_position_to_tick(word_pos, lsb as i32, tick_spacing);
            (tick, true)
        }
        None => {
            // No initialized tick in this word at or above current position
            // Return end of word
            let tick = bitmap_position_to_tick(word_pos, 127, tick_spacing);
            (tick, false)
        }
    }
}

// ============================================================================
// PURE TICK COMPUTATION FUNCTIONS
// ============================================================================

/// Compute new liquidity values after a tick update (pure)
/// Returns (liquidity_gross_after, liquidity_net_after, flipped, should_init_fee_growth)
pub fn compute_liquidity_after_update(
    liquidity_gross_before: u128,
    liquidity_net_before: i128,
    liquidity_delta: i128,
    upper: bool,
    max_liquidity: u128,
    tick: i32,
    tick_current: i32,
) -> (u128, i128, bool, bool) {
    // Calculate new gross liquidity
    let liquidity_gross_after = if liquidity_delta < 0 {
        liquidity_gross_before - ((-liquidity_delta) as u128)
    } else {
        liquidity_gross_before + (liquidity_delta as u128)
    };

    if liquidity_gross_after > max_liquidity {
        panic!("Liquidity overflow");
    }

    // Check if tick state flipped (initialized <-> uninitialized)
    let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

    // Should initialize fee growth only when first adding liquidity to a tick below current
    let should_init_fee_growth = liquidity_gross_before == 0 && tick <= tick_current;

    // Calculate new net liquidity
    let liquidity_net_after = if upper {
        liquidity_net_before - liquidity_delta
    } else {
        liquidity_net_before + liquidity_delta
    };

    (liquidity_gross_after, liquidity_net_after, flipped, should_init_fee_growth)
}

/// Compute fee growth values after crossing a tick (pure)
/// Returns (new_fee_growth_outside_0, new_fee_growth_outside_1)
///
/// When crossing a tick, fee_growth_outside is flipped relative to global
pub fn compute_fee_growth_after_cross(
    fee_growth_outside_0: u128,
    fee_growth_outside_1: u128,
    fee_growth_global_0: u128,
    fee_growth_global_1: u128,
) -> (u128, u128) {
    (
        fee_growth_global_0 - fee_growth_outside_0,
        fee_growth_global_1 - fee_growth_outside_1,
    )
}

/// Compute fee growth below a tick (pure)
pub fn compute_fee_growth_below(
    tick: i32,
    tick_current: i32,
    fee_growth_outside_0: u128,
    fee_growth_outside_1: u128,
    fee_growth_global_0: u128,
    fee_growth_global_1: u128,
) -> (u128, u128) {
    if tick_current >= tick {
        (fee_growth_outside_0, fee_growth_outside_1)
    } else {
        (
            fee_growth_global_0 - fee_growth_outside_0,
            fee_growth_global_1 - fee_growth_outside_1,
        )
    }
}

/// Compute fee growth above a tick (pure)
pub fn compute_fee_growth_above(
    tick: i32,
    tick_current: i32,
    fee_growth_outside_0: u128,
    fee_growth_outside_1: u128,
    fee_growth_global_0: u128,
    fee_growth_global_1: u128,
) -> (u128, u128) {
    if tick_current < tick {
        (fee_growth_outside_0, fee_growth_outside_1)
    } else {
        (
            fee_growth_global_0 - fee_growth_outside_0,
            fee_growth_global_1 - fee_growth_outside_1,
        )
    }
}

/// Compute fee growth inside a tick range (pure)
///
/// # Properties (for formal verification)
/// - fee_inside = global - below - above
/// - Result uses wrapping subtraction for Q128.128 math
pub fn compute_fee_growth_inside_pure(
    fee_growth_below_0: u128,
    fee_growth_below_1: u128,
    fee_growth_above_0: u128,
    fee_growth_above_1: u128,
    fee_growth_global_0: u128,
    fee_growth_global_1: u128,
) -> (u128, u128) {
    (
        fee_growth_global_0 - fee_growth_below_0 - fee_growth_above_0,
        fee_growth_global_1 - fee_growth_below_1 - fee_growth_above_1,
    )
}

// ============================================================================
// SIDE EFFECT FUNCTIONS - Storage operations
// ============================================================================

/// Update a tick with liquidity delta (side effect)
/// Returns true if the tick was flipped (initialized or uninitialized)
pub fn update(
    env: &Env,
    tick: i32,
    tick_current: i32,
    liquidity_delta: i128,
    fee_growth_global_0_x128: u128,
    fee_growth_global_1_x128: u128,
    upper: bool,
    max_liquidity: u128,
) -> bool {
    let mut info = get_tick(env, tick);

    // Pure computation
    let (liquidity_gross_after, liquidity_net_after, flipped, should_init_fee_growth) =
        compute_liquidity_after_update(
            info.liquidity_gross,
            info.liquidity_net,
            liquidity_delta,
            upper,
            max_liquidity,
            tick,
            tick_current,
        );

    // Apply state changes
    if should_init_fee_growth {
        info.fee_growth_outside_0_x128 = fee_growth_global_0_x128;
        info.fee_growth_outside_1_x128 = fee_growth_global_1_x128;
    }

    if info.liquidity_gross == 0 && liquidity_gross_after > 0 {
        info.initialized = true;
    }

    info.liquidity_gross = liquidity_gross_after;
    info.liquidity_net = liquidity_net_after;

    set_tick(env, tick, &info);

    flipped
}

/// Cross a tick during a swap (side effect)
/// Returns the liquidity delta to apply
pub fn cross(
    env: &Env,
    tick: i32,
    fee_growth_global_0_x128: u128,
    fee_growth_global_1_x128: u128,
) -> i128 {
    let mut info = get_tick(env, tick);

    // Pure computation
    let (new_fee_0, new_fee_1) = compute_fee_growth_after_cross(
        info.fee_growth_outside_0_x128,
        info.fee_growth_outside_1_x128,
        fee_growth_global_0_x128,
        fee_growth_global_1_x128,
    );

    // Apply state changes
    info.fee_growth_outside_0_x128 = new_fee_0;
    info.fee_growth_outside_1_x128 = new_fee_1;

    set_tick(env, tick, &info);

    info.liquidity_net
}

/// Get fee growth inside a tick range (side effect - reads storage)
pub fn get_fee_growth_inside(
    env: &Env,
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    fee_growth_global_0_x128: u128,
    fee_growth_global_1_x128: u128,
) -> (u128, u128) {
    let lower = get_tick(env, tick_lower);
    let upper = get_tick(env, tick_upper);

    // Pure computations
    let (fee_growth_below_0, fee_growth_below_1) = compute_fee_growth_below(
        tick_lower,
        tick_current,
        lower.fee_growth_outside_0_x128,
        lower.fee_growth_outside_1_x128,
        fee_growth_global_0_x128,
        fee_growth_global_1_x128,
    );

    let (fee_growth_above_0, fee_growth_above_1) = compute_fee_growth_above(
        tick_upper,
        tick_current,
        upper.fee_growth_outside_0_x128,
        upper.fee_growth_outside_1_x128,
        fee_growth_global_0_x128,
        fee_growth_global_1_x128,
    );

    compute_fee_growth_inside_pure(
        fee_growth_below_0,
        fee_growth_below_1,
        fee_growth_above_0,
        fee_growth_above_1,
        fee_growth_global_0_x128,
        fee_growth_global_1_x128,
    )
}

/// Flip a tick in the bitmap (side effect)
pub fn flip_tick(env: &Env, tick: i32, tick_spacing: i32) {
    if tick % tick_spacing != 0 {
        panic!("Tick not on spacing");
    }

    let (word_pos, bit_pos) = tick_to_bitmap_position(tick, tick_spacing);
    let mask = 1u128 << bit_pos;
    let word = get_tick_bitmap_word(env, word_pos);
    set_tick_bitmap_word(env, word_pos, word ^ mask);
}

/// Find the next initialized tick within one word (side effect - reads storage)
/// Returns (tick, initialized)
pub fn next_initialized_tick_within_one_word(
    env: &Env,
    tick: i32,
    tick_spacing: i32,
    lte: bool, // less than or equal (searching left)
) -> (i32, bool) {
    let compressed = tick / tick_spacing;

    if lte {
        let (word_pos, bit_pos) = tick_to_bitmap_position(tick, tick_spacing);
        let word = get_tick_bitmap_word(env, word_pos);
        compute_next_tick_lte(word, word_pos, bit_pos, tick_spacing)
    } else {
        // Search right (greater than)
        let compressed_plus_one = compressed + 1;
        let word_pos = compressed_plus_one >> 7;
        let bit_pos = (compressed_plus_one.rem_euclid(128)) as u8;
        let word = get_tick_bitmap_word(env, word_pos);
        compute_next_tick_gt(word, word_pos, bit_pos, tick_spacing)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{set_tick, set_tick_bitmap_word};
    use soroban_sdk::Env;

    /// Helper to run test code within a contract context
    fn with_contract<F, R>(env: &Env, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let contract_id = env.register_contract(None, crate::DexPool);
        env.as_contract(&contract_id, f)
    }

    // ============================================================================
    // PURE BITMAP FUNCTION TESTS
    // ============================================================================

    #[test]
    fn test_tick_to_bitmap_position_positive() {
        // tick = 60, spacing = 60 -> compressed = 1 -> word 0, bit 1
        let (word, bit) = tick_to_bitmap_position(60, 60);
        assert_eq!(word, 0);
        assert_eq!(bit, 1);
    }

    #[test]
    fn test_tick_to_bitmap_position_negative() {
        // tick = -60, spacing = 60 -> compressed = -1 -> word -1, bit 127
        let (word, bit) = tick_to_bitmap_position(-60, 60);
        assert_eq!(word, -1);
        assert_eq!(bit, 127);
    }

    #[test]
    fn test_tick_to_bitmap_position_zero() {
        let (word, bit) = tick_to_bitmap_position(0, 60);
        assert_eq!(word, 0);
        assert_eq!(bit, 0);
    }

    #[test]
    fn test_tick_to_bitmap_position_word_boundary() {
        // tick = 128 * 60 = 7680 -> compressed = 128 -> word 1, bit 0
        let (word, bit) = tick_to_bitmap_position(7680, 60);
        assert_eq!(word, 1);
        assert_eq!(bit, 0);
    }

    #[test]
    fn test_bitmap_position_roundtrip() {
        let tick_spacing = 60;
        for tick in [-7680, -60, 0, 60, 7680] {
            let (word, bit) = tick_to_bitmap_position(tick, tick_spacing);
            let recovered = bitmap_position_to_tick(word, bit as i32, tick_spacing);
            assert_eq!(recovered, tick, "Roundtrip failed for tick {}", tick);
        }
    }

    #[test]
    fn test_create_mask_at_or_below() {
        // bit_pos = 0 -> mask = 0b1
        assert_eq!(create_mask_at_or_below(0), 1);
        // bit_pos = 3 -> mask = 0b1111
        assert_eq!(create_mask_at_or_below(3), 0b1111);
        // bit_pos = 7 -> mask = 0xFF
        assert_eq!(create_mask_at_or_below(7), 0xFF);
    }

    #[test]
    fn test_create_mask_at_or_above() {
        // bit_pos = 0 -> all bits set
        assert_eq!(create_mask_at_or_above(0), u128::MAX);
        // bit_pos = 1 -> all bits except bit 0
        assert_eq!(create_mask_at_or_above(1), u128::MAX - 1);
        // bit_pos = 127 -> only bit 127
        assert_eq!(create_mask_at_or_above(127), 1u128 << 127);
    }

    #[test]
    fn test_find_most_significant_bit() {
        assert_eq!(find_most_significant_bit(0), None);
        assert_eq!(find_most_significant_bit(1), Some(0));
        assert_eq!(find_most_significant_bit(0b1000), Some(3));
        assert_eq!(find_most_significant_bit(0b1010), Some(3));
        assert_eq!(find_most_significant_bit(1u128 << 127), Some(127));
    }

    #[test]
    fn test_find_least_significant_bit() {
        assert_eq!(find_least_significant_bit(0), None);
        assert_eq!(find_least_significant_bit(1), Some(0));
        assert_eq!(find_least_significant_bit(0b1000), Some(3));
        assert_eq!(find_least_significant_bit(0b1010), Some(1));
        assert_eq!(find_least_significant_bit(1u128 << 127), Some(127));
    }

    #[test]
    fn test_compute_next_tick_lte_found() {
        let word = 1u128 << 5; // bit 5 set
        let (tick, initialized) = compute_next_tick_lte(word, 0, 10, 10);
        assert!(initialized);
        assert_eq!(tick, 50); // bit 5 * spacing 10
    }

    #[test]
    fn test_compute_next_tick_lte_not_found() {
        let word = 1u128 << 10; // bit 10 set, but we're at bit 5
        let (tick, initialized) = compute_next_tick_lte(word, 0, 5, 10);
        assert!(!initialized);
        assert_eq!(tick, 0); // word boundary
    }

    #[test]
    fn test_compute_next_tick_gt_found() {
        let word = 1u128 << 20; // bit 20 set
        let (tick, initialized) = compute_next_tick_gt(word, 0, 6, 10);
        assert!(initialized);
        assert_eq!(tick, 200); // bit 20 * spacing 10
    }

    #[test]
    fn test_compute_next_tick_gt_not_found() {
        let word = 1u128 << 5; // bit 5 set, but we're at bit 10
        let (tick, initialized) = compute_next_tick_gt(word, 0, 10, 10);
        assert!(!initialized);
        assert_eq!(tick, 1270); // word boundary (127 * 10)
    }

    // ============================================================================
    // PURE LIQUIDITY COMPUTATION TESTS
    // ============================================================================

    #[test]
    fn test_compute_liquidity_after_update_add() {
        let (gross, net, flipped, init_fee) =
            compute_liquidity_after_update(0, 0, 1000, false, u128::MAX, -100, 0);
        assert_eq!(gross, 1000);
        assert_eq!(net, 1000);
        assert!(flipped); // 0 -> non-zero
        assert!(init_fee); // tick -100 <= current 0
    }

    #[test]
    fn test_compute_liquidity_after_update_add_upper() {
        let (gross, net, flipped, _) =
            compute_liquidity_after_update(0, 0, 1000, true, u128::MAX, 100, 0);
        assert_eq!(gross, 1000);
        assert_eq!(net, -1000); // upper tick subtracts
        assert!(flipped);
    }

    #[test]
    fn test_compute_liquidity_after_update_remove() {
        let (gross, net, flipped, _) =
            compute_liquidity_after_update(1000, 1000, -500, false, u128::MAX, 0, 0);
        assert_eq!(gross, 500);
        assert_eq!(net, 500);
        assert!(!flipped); // still has liquidity
    }

    #[test]
    fn test_compute_liquidity_after_update_remove_all() {
        let (gross, _, flipped, _) =
            compute_liquidity_after_update(1000, 1000, -1000, false, u128::MAX, 0, 0);
        assert_eq!(gross, 0);
        assert!(flipped); // non-zero -> 0
    }

    #[test]
    #[should_panic(expected = "Liquidity overflow")]
    fn test_compute_liquidity_after_update_overflow() {
        compute_liquidity_after_update(0, 0, 1000, false, 500, 0, 0);
    }

    // ============================================================================
    // PURE FEE GROWTH COMPUTATION TESTS
    // ============================================================================

    #[test]
    fn test_compute_fee_growth_after_cross() {
        let (new_0, new_1) = compute_fee_growth_after_cross(100, 200, 1000, 2000);
        assert_eq!(new_0, 900); // 1000 - 100
        assert_eq!(new_1, 1800); // 2000 - 200
    }

    #[test]
    fn test_compute_fee_growth_below_current_above() {
        // Current tick >= lower tick -> use outside directly
        let (below_0, below_1) = compute_fee_growth_below(
            -100, 0, // tick, tick_current
            100, 200, // outside values
            1000, 2000, // global values
        );
        assert_eq!(below_0, 100);
        assert_eq!(below_1, 200);
    }

    #[test]
    fn test_compute_fee_growth_below_current_below() {
        // Current tick < lower tick -> flip
        let (below_0, below_1) = compute_fee_growth_below(
            100, 0, // tick, tick_current (current < tick)
            100, 200, // outside values
            1000, 2000, // global values
        );
        assert_eq!(below_0, 900); // global - outside
        assert_eq!(below_1, 1800);
    }

    #[test]
    fn test_compute_fee_growth_above_current_below() {
        // Current tick < upper tick -> use outside directly
        let (above_0, above_1) = compute_fee_growth_above(
            100, 0, // tick, tick_current
            50, 100, // outside values
            1000, 2000, // global values
        );
        assert_eq!(above_0, 50);
        assert_eq!(above_1, 100);
    }

    #[test]
    fn test_compute_fee_growth_above_current_above() {
        // Current tick >= upper tick -> flip
        let (above_0, above_1) = compute_fee_growth_above(
            -100, 0, // tick, tick_current (current >= tick)
            100, 200, // outside values
            1000, 2000, // global values
        );
        assert_eq!(above_0, 900); // global - outside
        assert_eq!(above_1, 1800);
    }

    #[test]
    fn test_compute_fee_growth_inside_pure() {
        let (inside_0, inside_1) = compute_fee_growth_inside_pure(
            100, 200, // below
            50, 100,  // above
            1000, 2000, // global
        );
        // inside = global - below - above
        assert_eq!(inside_0, 850); // 1000 - 100 - 50
        assert_eq!(inside_1, 1700); // 2000 - 200 - 100
    }

    // ============================================================================
    // STORAGE-BASED TESTS (existing tests, updated)
    // ============================================================================

    #[test]
    fn test_update_initializes_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 100;
            let tick_current = 0;
            let liquidity_delta = 1000i128;
            let max_liquidity = u128::MAX;

            let flipped = update(
                &env,
                tick,
                tick_current,
                liquidity_delta,
                0,
                0,
                false,
                max_liquidity,
            );

            assert!(flipped);

            let info = get_tick(&env, tick);
            assert!(info.initialized);
            assert_eq!(info.liquidity_gross, 1000);
            assert_eq!(info.liquidity_net, 1000);
        });
    }

    #[test]
    fn test_update_add_liquidity_lower_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = -100;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            let flipped = update(&env, tick, tick_current, 500, 0, 0, false, max_liquidity);
            assert!(!flipped);

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 1500);
            assert_eq!(info.liquidity_net, 1500);
        });
    }

    #[test]
    fn test_update_add_liquidity_upper_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 100;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            update(&env, tick, tick_current, 1000, 0, 0, true, max_liquidity);

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 1000);
            assert_eq!(info.liquidity_net, -1000);
        });
    }

    #[test]
    fn test_update_remove_liquidity() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            let flipped = update(&env, tick, tick_current, -400, 0, 0, false, max_liquidity);
            assert!(!flipped);

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 600);
            assert_eq!(info.liquidity_net, 600);
        });
    }

    #[test]
    fn test_update_remove_all_liquidity_flips() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            let flipped = update(&env, tick, tick_current, -1000, 0, 0, false, max_liquidity);
            assert!(flipped);

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 0);
        });
    }

    #[test]
    fn test_update_initializes_fee_growth_below_current() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = -100;
            let tick_current = 0;
            let fee_growth_0 = 1000u128;
            let fee_growth_1 = 2000u128;
            let max_liquidity = u128::MAX;

            update(
                &env,
                tick,
                tick_current,
                1000,
                fee_growth_0,
                fee_growth_1,
                false,
                max_liquidity,
            );

            let info = get_tick(&env, tick);
            assert_eq!(info.fee_growth_outside_0_x128, fee_growth_0);
            assert_eq!(info.fee_growth_outside_1_x128, fee_growth_1);
        });
    }

    #[test]
    fn test_update_does_not_initialize_fee_growth_above_current() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 100;
            let tick_current = 0;
            let fee_growth_0 = 1000u128;
            let fee_growth_1 = 2000u128;
            let max_liquidity = u128::MAX;

            update(
                &env,
                tick,
                tick_current,
                1000,
                fee_growth_0,
                fee_growth_1,
                false,
                max_liquidity,
            );

            let info = get_tick(&env, tick);
            assert_eq!(info.fee_growth_outside_0_x128, 0);
            assert_eq!(info.fee_growth_outside_1_x128, 0);
        });
    }

    #[test]
    #[should_panic(expected = "Liquidity overflow")]
    fn test_update_exceeds_max_liquidity() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;
            let tick_current = 0;
            let max_liquidity = 1000u128;

            update(&env, tick, tick_current, 2000, 0, 0, false, max_liquidity);
        });
    }

    #[test]
    fn test_cross_flips_fee_growth() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;

            let info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 500,
                fee_growth_outside_0_x128: 100,
                fee_growth_outside_1_x128: 200,
                initialized: true,
            };
            set_tick(&env, tick, &info);

            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            let liquidity_net = cross(&env, tick, fee_global_0, fee_global_1);

            assert_eq!(liquidity_net, 500);

            let new_info = get_tick(&env, tick);
            assert_eq!(new_info.fee_growth_outside_0_x128, fee_global_0 - 100);
            assert_eq!(new_info.fee_growth_outside_1_x128, fee_global_1 - 200);
        });
    }

    #[test]
    fn test_cross_returns_liquidity_net() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;

            let info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -750,
                fee_growth_outside_0_x128: 0,
                fee_growth_outside_1_x128: 0,
                initialized: true,
            };
            set_tick(&env, tick, &info);

            let liquidity_net = cross(&env, tick, 0, 0);
            assert_eq!(liquidity_net, -750);
        });
    }

    #[test]
    fn test_get_fee_growth_inside_current_in_range() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = -100;
            let tick_upper = 100;
            let tick_current = 0;
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            let lower_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 1000,
                fee_growth_outside_0_x128: 100,
                fee_growth_outside_1_x128: 200,
                initialized: true,
            };
            let upper_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -1000,
                fee_growth_outside_0_x128: 50,
                fee_growth_outside_1_x128: 100,
                initialized: true,
            };
            set_tick(&env, tick_lower, &lower_info);
            set_tick(&env, tick_upper, &upper_info);

            let (fee_inside_0, fee_inside_1) = get_fee_growth_inside(
                &env,
                tick_lower,
                tick_upper,
                tick_current,
                fee_global_0,
                fee_global_1,
            );

            let expected_0 = fee_global_0 - 100 - 50;
            let expected_1 = fee_global_1 - 200 - 100;
            assert_eq!(fee_inside_0, expected_0);
            assert_eq!(fee_inside_1, expected_1);
        });
    }

    #[test]
    fn test_get_fee_growth_inside_current_below_range() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = 100;
            let tick_upper = 200;
            let tick_current = 0;
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            let lower_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 1000,
                fee_growth_outside_0_x128: 800,
                fee_growth_outside_1_x128: 1600,
                initialized: true,
            };
            let upper_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -1000,
                fee_growth_outside_0_x128: 300,
                fee_growth_outside_1_x128: 600,
                initialized: true,
            };
            set_tick(&env, tick_lower, &lower_info);
            set_tick(&env, tick_upper, &upper_info);

            let (fee_inside_0, fee_inside_1) = get_fee_growth_inside(
                &env,
                tick_lower,
                tick_upper,
                tick_current,
                fee_global_0,
                fee_global_1,
            );

            let fee_below_0 = fee_global_0 - lower_info.fee_growth_outside_0_x128;
            let fee_below_1 = fee_global_1 - lower_info.fee_growth_outside_1_x128;
            let fee_above_0 = upper_info.fee_growth_outside_0_x128;
            let fee_above_1 = upper_info.fee_growth_outside_1_x128;
            let expected_0 = fee_global_0 - fee_below_0 - fee_above_0;
            let expected_1 = fee_global_1 - fee_below_1 - fee_above_1;
            assert_eq!(fee_inside_0, expected_0);
            assert_eq!(fee_inside_1, expected_1);
        });
    }

    #[test]
    fn test_get_fee_growth_inside_current_above_range() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = -200;
            let tick_upper = -100;
            let tick_current = 0;
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            let lower_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 1000,
                fee_growth_outside_0_x128: 100,
                fee_growth_outside_1_x128: 200,
                initialized: true,
            };
            let upper_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -1000,
                fee_growth_outside_0_x128: 600,
                fee_growth_outside_1_x128: 1200,
                initialized: true,
            };
            set_tick(&env, tick_lower, &lower_info);
            set_tick(&env, tick_upper, &upper_info);

            let (fee_inside_0, fee_inside_1) = get_fee_growth_inside(
                &env,
                tick_lower,
                tick_upper,
                tick_current,
                fee_global_0,
                fee_global_1,
            );

            let fee_below_0 = lower_info.fee_growth_outside_0_x128;
            let fee_below_1 = lower_info.fee_growth_outside_1_x128;
            let fee_above_0 = fee_global_0 - upper_info.fee_growth_outside_0_x128;
            let fee_above_1 = fee_global_1 - upper_info.fee_growth_outside_1_x128;
            let expected_0 = fee_global_0 - fee_below_0 - fee_above_0;
            let expected_1 = fee_global_1 - fee_below_1 - fee_above_1;
            assert_eq!(fee_inside_0, expected_0);
            assert_eq!(fee_inside_1, expected_1);
        });
    }

    #[test]
    fn test_flip_tick_sets_bit() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 60;
            let tick_spacing = 60;

            flip_tick(&env, tick, tick_spacing);

            let word = get_tick_bitmap_word(&env, 0);
            assert_eq!(word, 1u128 << 1);
        });
    }

    #[test]
    fn test_flip_tick_clears_bit() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 60;
            let tick_spacing = 60;

            flip_tick(&env, tick, tick_spacing);
            flip_tick(&env, tick, tick_spacing);

            let word = get_tick_bitmap_word(&env, 0);
            assert_eq!(word, 0);
        });
    }

    #[test]
    fn test_flip_tick_negative() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = -60;
            let tick_spacing = 60;

            flip_tick(&env, tick, tick_spacing);

            let word = get_tick_bitmap_word(&env, -1);
            assert_eq!(word, 1u128 << 127);
        });
    }

    #[test]
    fn test_flip_tick_multiple_ticks() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            flip_tick(&env, 0, tick_spacing);
            flip_tick(&env, 10, tick_spacing);
            flip_tick(&env, 20, tick_spacing);

            let word = get_tick_bitmap_word(&env, 0);
            let expected = (1u128 << 0) | (1u128 << 1) | (1u128 << 2);
            assert_eq!(word, expected);
        });
    }

    #[test]
    #[should_panic(expected = "Tick not on spacing")]
    fn test_flip_tick_not_on_spacing() {
        let env = Env::default();
        with_contract(&env, || {
            flip_tick(&env, 15, 10);
        });
    }

    #[test]
    fn test_next_initialized_tick_lte_finds_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            set_tick_bitmap_word(&env, 0, 1u128 << 5);

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 100, tick_spacing, true);

            assert!(initialized);
            assert_eq!(next, 50);
        });
    }

    #[test]
    fn test_next_initialized_tick_lte_no_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 100, tick_spacing, true);

            assert!(!initialized);
            assert_eq!(next, 0);
        });
    }

    #[test]
    fn test_next_initialized_tick_gt_finds_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            set_tick_bitmap_word(&env, 0, 1u128 << 20);

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 50, tick_spacing, false);

            assert!(initialized);
            assert_eq!(next, 200);
        });
    }

    #[test]
    fn test_next_initialized_tick_gt_no_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 50, tick_spacing, false);

            assert!(!initialized);
            assert_eq!(next, 127 * tick_spacing);
        });
    }

    #[test]
    fn test_next_initialized_tick_at_boundary() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            set_tick_bitmap_word(&env, 0, 1u128 << 5);

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 50, tick_spacing, true);

            assert!(initialized);
            assert_eq!(next, 50);
        });
    }

    #[test]
    fn test_next_initialized_tick_negative_ticks() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            set_tick_bitmap_word(&env, -1, 1u128 << 118);

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, -50, tick_spacing, true);

            assert!(initialized);
            assert_eq!(next, -100);
        });
    }

    #[test]
    fn test_position_lifecycle() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = -100;
            let tick_upper = 100;
            let tick_current = 0;
            let tick_spacing = 10;
            let max_liquidity = u128::MAX;

            let flipped_lower = update(
                &env,
                tick_lower,
                tick_current,
                1000,
                0,
                0,
                false,
                max_liquidity,
            );
            let flipped_upper = update(
                &env,
                tick_upper,
                tick_current,
                1000,
                0,
                0,
                true,
                max_liquidity,
            );
            assert!(flipped_lower);
            assert!(flipped_upper);

            flip_tick(&env, tick_lower, tick_spacing);
            flip_tick(&env, tick_upper, tick_spacing);

            let flipped_lower = update(
                &env,
                tick_lower,
                tick_current,
                -1000,
                0,
                0,
                false,
                max_liquidity,
            );
            let flipped_upper = update(
                &env,
                tick_upper,
                tick_current,
                -1000,
                0,
                0,
                true,
                max_liquidity,
            );
            assert!(flipped_lower);
            assert!(flipped_upper);

            let lower_info = get_tick(&env, tick_lower);
            let upper_info = get_tick(&env, tick_upper);
            assert_eq!(lower_info.liquidity_gross, 0);
            assert_eq!(upper_info.liquidity_gross, 0);
        });
    }
}
