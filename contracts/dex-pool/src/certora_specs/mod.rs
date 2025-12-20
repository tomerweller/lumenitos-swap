// ============================================================================
// CERTORA SUNBEAM FORMAL VERIFICATION SPECIFICATIONS
// ============================================================================
//
// This module contains formal verification specifications for the Lumenitos Swap
// concentrated liquidity DEX, based on Uniswap v3 invariants.
//
// STRUCTURE (following Certora best practices):
//
// - model.rs      : Ghost state, Skolem variables, state snapshots
// - math_specs.rs : Pure math function verification
// - pool_state_specs.rs : Pool initialization and state invariants
// - swap_specs.rs : Swap operation verification
// - liquidity_specs.rs : Mint/burn/collect verification
// - tick_specs.rs : Tick management verification
//
// PATTERNS USED:
//
// 1. Ghost state - Track properties across function calls
// 2. Skolem variables - Prove universal properties for arbitrary values
// 3. State snapshots - Before/after comparisons
// 4. Sanity rules - Ensure rules aren't vacuously true
// 5. Model initialization - Set up nondeterministic initial state
//
// USAGE:
// - Unit tests: cargo test -p dex-pool
// - Certora build: cargo build --features certora -p dex-pool
// - Verification: certoraSorobanProver dex_pool.conf
//
// ============================================================================

// Ghost state and model initialization
#[cfg(feature = "certora")]
pub mod model;

// Spec modules
pub mod math_specs;
pub mod pool_state_specs;
pub mod swap_specs;
pub mod liquidity_specs;
pub mod tick_specs;
