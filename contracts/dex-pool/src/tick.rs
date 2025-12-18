use crate::storage::{get_tick, get_tick_bitmap_word, set_tick, set_tick_bitmap_word};
use soroban_sdk::Env;

/// Update a tick with liquidity delta
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

    let liquidity_gross_before = info.liquidity_gross;
    let liquidity_gross_after = if liquidity_delta < 0 {
        liquidity_gross_before - ((-liquidity_delta) as u128)
    } else {
        liquidity_gross_before + (liquidity_delta as u128)
    };

    if liquidity_gross_after > max_liquidity {
        panic!("Liquidity overflow");
    }

    let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

    if liquidity_gross_before == 0 {
        // Initialize tick
        if tick <= tick_current {
            info.fee_growth_outside_0_x128 = fee_growth_global_0_x128;
            info.fee_growth_outside_1_x128 = fee_growth_global_1_x128;
        }
        info.initialized = true;
    }

    info.liquidity_gross = liquidity_gross_after;

    // Update liquidity_net (add for lower tick, subtract for upper tick)
    info.liquidity_net = if upper {
        info.liquidity_net - liquidity_delta
    } else {
        info.liquidity_net + liquidity_delta
    };

    set_tick(env, tick, &info);

    flipped
}

/// Cross a tick during a swap
/// Returns the liquidity delta to apply
pub fn cross(
    env: &Env,
    tick: i32,
    fee_growth_global_0_x128: u128,
    fee_growth_global_1_x128: u128,
) -> i128 {
    let mut info = get_tick(env, tick);

    // Flip fee growth outside
    info.fee_growth_outside_0_x128 = fee_growth_global_0_x128 - info.fee_growth_outside_0_x128;
    info.fee_growth_outside_1_x128 = fee_growth_global_1_x128 - info.fee_growth_outside_1_x128;

    set_tick(env, tick, &info);

    info.liquidity_net
}

/// Get fee growth inside a tick range
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

    // Calculate fee growth below
    let (fee_growth_below_0, fee_growth_below_1) = if tick_current >= tick_lower {
        (lower.fee_growth_outside_0_x128, lower.fee_growth_outside_1_x128)
    } else {
        (
            fee_growth_global_0_x128 - lower.fee_growth_outside_0_x128,
            fee_growth_global_1_x128 - lower.fee_growth_outside_1_x128,
        )
    };

    // Calculate fee growth above
    let (fee_growth_above_0, fee_growth_above_1) = if tick_current < tick_upper {
        (upper.fee_growth_outside_0_x128, upper.fee_growth_outside_1_x128)
    } else {
        (
            fee_growth_global_0_x128 - upper.fee_growth_outside_0_x128,
            fee_growth_global_1_x128 - upper.fee_growth_outside_1_x128,
        )
    };

    // Fee growth inside = global - below - above
    (
        fee_growth_global_0_x128 - fee_growth_below_0 - fee_growth_above_0,
        fee_growth_global_1_x128 - fee_growth_below_1 - fee_growth_above_1,
    )
}

// === Tick Bitmap Operations ===
// Using u128 per word (128 ticks per word)

/// Flip a tick in the bitmap
pub fn flip_tick(env: &Env, tick: i32, tick_spacing: i32) {
    if tick % tick_spacing != 0 {
        panic!("Tick not on spacing");
    }

    let compressed = tick / tick_spacing;
    let word_pos = compressed >> 7; // divide by 128
    let bit_pos = (compressed.rem_euclid(128)) as u8;

    let mask = 1u128 << bit_pos;
    let word = get_tick_bitmap_word(env, word_pos);
    set_tick_bitmap_word(env, word_pos, word ^ mask);
}

/// Find the next initialized tick within one word
/// Returns (tick, initialized)
pub fn next_initialized_tick_within_one_word(
    env: &Env,
    tick: i32,
    tick_spacing: i32,
    lte: bool, // less than or equal (searching left)
) -> (i32, bool) {
    let compressed = tick / tick_spacing;

    if lte {
        let word_pos = compressed >> 7;
        let bit_pos = (compressed.rem_euclid(128)) as u8;

        // Create mask for bits at or below current position
        let mask = (1u128 << bit_pos) - 1 + (1u128 << bit_pos);
        let masked = get_tick_bitmap_word(env, word_pos) & mask;

        let initialized = masked != 0;
        let next = if initialized {
            let msb = 127 - masked.leading_zeros() as i32;
            ((word_pos * 128) + msb) * tick_spacing
        } else {
            (word_pos * 128) * tick_spacing
        };

        (next, initialized)
    } else {
        // Search right (greater than)
        let compressed_plus_one = compressed + 1;
        let word_pos = compressed_plus_one >> 7;
        let bit_pos = (compressed_plus_one.rem_euclid(128)) as u8;

        // Create mask for bits at or above current position
        let mask = !((1u128 << bit_pos) - 1);
        let masked = get_tick_bitmap_word(env, word_pos) & mask;

        let initialized = masked != 0;
        let next = if initialized {
            let lsb = masked.trailing_zeros() as i32;
            ((word_pos * 128) + lsb) * tick_spacing
        } else {
            ((word_pos * 128) + 127) * tick_spacing
        };

        (next, initialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{set_tick, set_tick_bitmap_word};
    use dex_types::TickInfo;
    use soroban_sdk::Env;

    /// Helper to run test code within a contract context
    fn with_contract<F, R>(env: &Env, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let contract_id = env.register_contract(None, crate::DexPool);
        env.as_contract(&contract_id, f)
    }

    // === update tests ===

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
                false, // lower tick
                max_liquidity,
            );

            assert!(flipped, "First liquidity addition should flip tick");

            let info = get_tick(&env, tick);
            assert!(info.initialized, "Tick should be initialized");
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

            // Add initial liquidity
            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            // Add more liquidity
            let flipped = update(&env, tick, tick_current, 500, 0, 0, false, max_liquidity);
            assert!(!flipped, "Adding more liquidity should not flip");

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 1500);
            assert_eq!(info.liquidity_net, 1500, "Lower tick adds to liquidity_net");
        });
    }

    #[test]
    fn test_update_add_liquidity_upper_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 100;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            // Add liquidity at upper tick
            update(&env, tick, tick_current, 1000, 0, 0, true, max_liquidity);

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 1000);
            assert_eq!(
                info.liquidity_net, -1000,
                "Upper tick subtracts from liquidity_net"
            );
        });
    }

    #[test]
    fn test_update_remove_liquidity() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;
            let tick_current = 0;
            let max_liquidity = u128::MAX;

            // Add liquidity
            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            // Remove partial liquidity
            let flipped = update(&env, tick, tick_current, -400, 0, 0, false, max_liquidity);
            assert!(!flipped, "Partial removal should not flip");

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

            // Add liquidity
            update(&env, tick, tick_current, 1000, 0, 0, false, max_liquidity);

            // Remove all liquidity
            let flipped = update(&env, tick, tick_current, -1000, 0, 0, false, max_liquidity);
            assert!(flipped, "Removing all liquidity should flip tick");

            let info = get_tick(&env, tick);
            assert_eq!(info.liquidity_gross, 0);
        });
    }

    #[test]
    fn test_update_initializes_fee_growth_below_current() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = -100; // Below current tick
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
            assert_eq!(
                info.fee_growth_outside_0_x128, fee_growth_0,
                "Fee growth should be initialized when below current"
            );
            assert_eq!(info.fee_growth_outside_1_x128, fee_growth_1);
        });
    }

    #[test]
    fn test_update_does_not_initialize_fee_growth_above_current() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 100; // Above current tick
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
            assert_eq!(
                info.fee_growth_outside_0_x128, 0,
                "Fee growth should not be initialized when above current"
            );
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

    // === cross tests ===

    #[test]
    fn test_cross_flips_fee_growth() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 0;

            // Set up tick with fee growth
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
            assert_eq!(
                new_info.fee_growth_outside_0_x128,
                fee_global_0 - 100,
                "Fee growth should be flipped"
            );
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

    // === get_fee_growth_inside tests ===

    #[test]
    fn test_get_fee_growth_inside_current_in_range() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = -100;
            let tick_upper = 100;
            let tick_current = 0;
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            // Initialize ticks
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

            // fee_inside = global - below - above
            // below = outside (when current >= lower)
            // above = outside (when current < upper)
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
            let tick_current = 0; // Below range
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            // When current < lower, fee_growth_outside represents fees above the tick
            // For lower tick: outside = fees above lower tick (i.e., fees in and above range)
            // For upper tick: outside = fees above upper tick (i.e., fees above range)
            let lower_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 1000,
                fee_growth_outside_0_x128: 800, // Fees above lower tick
                fee_growth_outside_1_x128: 1600,
                initialized: true,
            };
            let upper_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -1000,
                fee_growth_outside_0_x128: 300, // Fees above upper tick
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

            // When current < lower:
            // fee_below = global - lower.outside = 1000 - 800 = 200
            // fee_above = upper.outside = 300
            // fee_inside = global - below - above = 1000 - 200 - 300 = 500
            let fee_below_0 = fee_global_0 - lower_info.fee_growth_outside_0_x128; // 200
            let fee_below_1 = fee_global_1 - lower_info.fee_growth_outside_1_x128; // 400
            let fee_above_0 = upper_info.fee_growth_outside_0_x128; // 300
            let fee_above_1 = upper_info.fee_growth_outside_1_x128; // 600
            let expected_0 = fee_global_0 - fee_below_0 - fee_above_0; // 1000 - 200 - 300 = 500
            let expected_1 = fee_global_1 - fee_below_1 - fee_above_1; // 2000 - 400 - 600 = 1000
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
            let tick_current = 0; // Above range
            let fee_global_0 = 1000u128;
            let fee_global_1 = 2000u128;

            // When current >= upper, fee_growth_outside represents fees below the tick
            // For lower tick: outside = fees below lower tick
            // For upper tick: outside = fees below upper tick (i.e., fees in and below range)
            let lower_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: 1000,
                fee_growth_outside_0_x128: 100, // Fees below lower tick
                fee_growth_outside_1_x128: 200,
                initialized: true,
            };
            let upper_info = TickInfo {
                liquidity_gross: 1000,
                liquidity_net: -1000,
                fee_growth_outside_0_x128: 600, // Fees below upper tick (includes fees inside range)
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

            // When current >= upper:
            // fee_below = lower.outside = 100
            // fee_above = global - upper.outside = 1000 - 600 = 400
            // fee_inside = global - below - above = 1000 - 100 - 400 = 500
            let fee_below_0 = lower_info.fee_growth_outside_0_x128; // 100
            let fee_below_1 = lower_info.fee_growth_outside_1_x128; // 200
            let fee_above_0 = fee_global_0 - upper_info.fee_growth_outside_0_x128; // 400
            let fee_above_1 = fee_global_1 - upper_info.fee_growth_outside_1_x128; // 800
            let expected_0 = fee_global_0 - fee_below_0 - fee_above_0; // 500
            let expected_1 = fee_global_1 - fee_below_1 - fee_above_1; // 1000
            assert_eq!(fee_inside_0, expected_0);
            assert_eq!(fee_inside_1, expected_1);
        });
    }

    // === flip_tick tests ===

    #[test]
    fn test_flip_tick_sets_bit() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 60; // On tick spacing 60
            let tick_spacing = 60;

            flip_tick(&env, tick, tick_spacing);

            // tick/spacing = 1, word_pos = 0, bit_pos = 1
            let word = get_tick_bitmap_word(&env, 0);
            assert_eq!(word, 1u128 << 1, "Bit 1 should be set");
        });
    }

    #[test]
    fn test_flip_tick_clears_bit() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = 60;
            let tick_spacing = 60;

            // Set bit first
            flip_tick(&env, tick, tick_spacing);
            // Flip again to clear
            flip_tick(&env, tick, tick_spacing);

            let word = get_tick_bitmap_word(&env, 0);
            assert_eq!(word, 0, "Bit should be cleared after double flip");
        });
    }

    #[test]
    fn test_flip_tick_negative() {
        let env = Env::default();
        with_contract(&env, || {
            let tick = -60;
            let tick_spacing = 60;

            flip_tick(&env, tick, tick_spacing);

            // tick/spacing = -1, compressed.rem_euclid(128) = 127
            let word = get_tick_bitmap_word(&env, -1);
            assert_eq!(word, 1u128 << 127, "Bit 127 in word -1 should be set");
        });
    }

    #[test]
    fn test_flip_tick_multiple_ticks() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            // Flip ticks at 0, 10, 20
            flip_tick(&env, 0, tick_spacing);
            flip_tick(&env, 10, tick_spacing);
            flip_tick(&env, 20, tick_spacing);

            let word = get_tick_bitmap_word(&env, 0);
            // Bits 0, 1, 2 should be set
            let expected = (1u128 << 0) | (1u128 << 1) | (1u128 << 2);
            assert_eq!(word, expected);
        });
    }

    #[test]
    #[should_panic(expected = "Tick not on spacing")]
    fn test_flip_tick_not_on_spacing() {
        let env = Env::default();
        with_contract(&env, || {
            flip_tick(&env, 15, 10); // 15 is not divisible by 10
        });
    }

    // === next_initialized_tick_within_one_word tests ===

    #[test]
    fn test_next_initialized_tick_lte_finds_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            // Set up initialized tick at 50
            set_tick_bitmap_word(&env, 0, 1u128 << 5); // bit 5 = tick 50

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

            // No ticks set
            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 100, tick_spacing, true);

            assert!(!initialized);
            // Should return word boundary
            assert_eq!(next, 0);
        });
    }

    #[test]
    fn test_next_initialized_tick_gt_finds_tick() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            // Set up initialized tick at 200
            set_tick_bitmap_word(&env, 0, 1u128 << 20); // bit 20 = tick 200

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

            // No ticks set
            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 50, tick_spacing, false);

            assert!(!initialized);
            // Should return end of word
            assert_eq!(next, 127 * tick_spacing);
        });
    }

    #[test]
    fn test_next_initialized_tick_at_boundary() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            // Set tick at current position
            set_tick_bitmap_word(&env, 0, 1u128 << 5); // bit 5 = tick 50

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, 50, tick_spacing, true);

            assert!(initialized);
            assert_eq!(next, 50, "Should find tick at current position");
        });
    }

    #[test]
    fn test_next_initialized_tick_negative_ticks() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_spacing = 10;

            // Set tick at -100
            // -100 / 10 = -10, word_pos = -1, bit_pos = 118 (since -10 rem 128 = 118)
            set_tick_bitmap_word(&env, -1, 1u128 << 118);

            let (next, initialized) =
                next_initialized_tick_within_one_word(&env, -50, tick_spacing, true);

            assert!(initialized);
            assert_eq!(next, -100);
        });
    }

    // === Integration-style tests ===

    #[test]
    fn test_position_lifecycle() {
        let env = Env::default();
        with_contract(&env, || {
            let tick_lower = -100;
            let tick_upper = 100;
            let tick_current = 0;
            let tick_spacing = 10;
            let max_liquidity = u128::MAX;

            // 1. Add liquidity (opens position)
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

            // Flip ticks in bitmap
            flip_tick(&env, tick_lower, tick_spacing);
            flip_tick(&env, tick_upper, tick_spacing);

            // 2. Remove liquidity (closes position)
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

            // Verify ticks are cleared
            let lower_info = get_tick(&env, tick_lower);
            let upper_info = get_tick(&env, tick_upper);
            assert_eq!(lower_info.liquidity_gross, 0);
            assert_eq!(upper_info.liquidity_gross, 0);
        });
    }
}
