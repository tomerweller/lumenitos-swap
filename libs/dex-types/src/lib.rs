#![no_std]

mod pool;
mod position;
mod tick;

pub use pool::*;
pub use position::*;
pub use tick::*;

/// Q96 constant (2^96) for fixed-point math
pub const Q96: u128 = 1 << 96;

/// Minimum tick index
/// Limited by u128 representation (originally -887272 for uint160)
pub const MIN_TICK: i32 = -443636;

/// Maximum tick index
/// Limited by u128 representation (originally 887272 for uint160)
pub const MAX_TICK: i32 = 443636;

/// Minimum sqrt price (at MIN_TICK)
/// sqrt(1.0001^-443636) * 2^96
pub const MIN_SQRT_RATIO: u128 = 18446743374134;

/// Maximum sqrt price (at MAX_TICK)
/// sqrt(1.0001^443636) * 2^96, bounded by u128::MAX
pub const MAX_SQRT_RATIO: u128 = 340275971719517849884101479065584693834;

/// Fee amount in hundredths of a basis point (1e-6)
/// 500 = 0.05%, 3000 = 0.3%, 10000 = 1%
pub type Fee = u32;

/// Get tick spacing for a given fee amount
pub fn fee_to_tick_spacing(fee: Fee) -> i32 {
    match fee {
        500 => 10,    // 0.05%
        3000 => 60,   // 0.3%
        10000 => 200, // 1%
        _ => panic!("Invalid fee"),
    }
}

/// Calculate maximum liquidity per tick for a given tick spacing
pub fn max_liquidity_per_tick(tick_spacing: i32) -> u128 {
    let min_tick = (MIN_TICK / tick_spacing) * tick_spacing;
    let max_tick = (MAX_TICK / tick_spacing) * tick_spacing;
    let num_ticks = ((max_tick - min_tick) / tick_spacing) as u128 + 1;
    u128::MAX / num_ticks
}
