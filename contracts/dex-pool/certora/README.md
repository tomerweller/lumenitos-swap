# Certora Sunbeam Formal Verification

This directory contains configuration for formal verification using Certora Sunbeam.

## Current Status

✅ **Ready for Formal Verification**: All dependencies are correctly configured and the project builds with formal verification rules.

## What's Included

### Formal Verification Rules (in `src/certora_specs/`)

The following rules are defined using Certora's CVLR framework:

1. **Math Specs** (`math_specs.rs`) - 8 rules
   - `sanity_sqrt_ratio` - Reachability check
   - `sqrt_ratio_monotonic` - Price monotonicity with tick
   - `sqrt_ratio_min_bound` / `sqrt_ratio_max_bound` - Price bounds
   - `tick_roundtrip_consistent` - Tick ↔ price consistency
   - `mul_div_rounds_down` - Rounding behavior
   - `add_delta_positive_increases` / `add_delta_negative_decreases` - Delta operations

2. **Pool State Specs** (`pool_state_specs.rs`) - 5 rules
   - `price_always_in_bounds` / `tick_always_in_bounds` - State bounds
   - `fee_growth_monotonic` - Fee growth only increases
   - `fee_in_valid_range` / `tick_spacing_positive` - Config validity

3. **Swap Specs** (`swap_specs.rs`) - 5 rules
   - `swap_amounts_opposite_signs` - One in, one out
   - `zero_for_one_decreases_price` / `one_for_zero_increases_price` - Price direction
   - `swap_respects_price_limit` - Limit enforcement
   - `tick_crossings_bounded` - DoS protection

4. **Liquidity Specs** (`liquidity_specs.rs`) - 7 rules
   - `position_tick_bounds_valid` - Lower < upper
   - `position_ticks_aligned` - Tick spacing alignment
   - `liquidity_addition_safe` / `liquidity_subtraction_safe` - Overflow protection
   - `burn_bounded_by_position` / `collect_bounded_by_owed` - Amount bounds
   - `liquidity_net_balance` - Conservation at tick boundaries

5. **Tick Specs** (`tick_specs.rs`) - 8 rules
   - `tick_in_valid_range` / `tick_spacing_valid` - Tick bounds
   - `tick_aligned_to_spacing` - Alignment check
   - `liquidity_net_bounded_by_gross` - Net ≤ gross
   - `tick_initialized_iff_has_liquidity` / `bitmap_consistent_with_tick_init` - Bitmap consistency
   - `next_tick_respects_direction` - Search direction
   - `fee_tier_tick_spacing_relationship` - Fee/spacing mapping

### Configuration Files

- `certora_build.py` - Build script for Certora prover
- `dex_pool.conf` - Prover configuration with all rules

## Running Unit Tests

The invariants are also validated through unit tests:

```bash
cargo test -p dex-pool certora_specs
```

## Building with Certora Feature

```bash
cargo build --release --target wasm32-unknown-unknown -p dex-pool --features certora
```

## Running Formal Verification

1. **Install Certora CLI**:
   ```bash
   pip3 install certora-cli
   ```

2. **Set API Key**:
   ```bash
   export CERTORAKEY=<your_key>
   ```

3. **Run Prover**:
   ```bash
   cd contracts/dex-pool/certora
   certoraSorobanProver dex_pool.conf
   ```

## Dependencies

The workspace uses GitHub-sourced packages:

```toml
# In workspace Cargo.toml
cvlr = { git = "https://github.com/Certora/cvlr.git", default-features = false }
cvlr-soroban = { git = "https://github.com/Certora/cvlr-soroban.git", branch = "main" }
cvlr-soroban-macros = { git = "https://github.com/Certora/cvlr-soroban.git", branch = "main" }
cvlr-soroban-derive = { git = "https://github.com/Certora/cvlr-soroban.git", branch = "main" }
```

Key insight: `cvlr` with `default-features = false` is `no_std` compatible for Soroban.

## Rule Syntax

Rules use the following pattern:

```rust
#[cfg(feature = "certora")]
use cvlr_soroban_derive::rule;

#[cfg(feature = "certora")]
use cvlr::asserts::{cvlr_assert, cvlr_assume, cvlr_satisfy};

#[cfg(feature = "certora")]
#[rule]
pub fn my_rule(env: Env, param: Type) {
    cvlr_assume!(precondition);
    // ... logic ...
    cvlr_assert!(postcondition);
}
```

## Resources

- [Certora Sunbeam Documentation](https://docs.certora.com/en/latest/docs/sunbeam/index.html)
- [Certora Sunbeam Tutorials](https://github.com/Certora/sunbeam-tutorials)
- [cvlr-soroban Repository](https://github.com/Certora/cvlr-soroban)
- [cvlr Repository](https://github.com/Certora/cvlr)
