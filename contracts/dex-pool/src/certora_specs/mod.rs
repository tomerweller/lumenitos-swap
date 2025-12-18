// ============================================================================
// CERTORA SUNBEAM FORMAL VERIFICATION SPECIFICATIONS
// ============================================================================
//
// This module contains formal verification specifications for the Lumenitos Swap
// concentrated liquidity DEX, based on Uniswap v3 invariants.
//
// INVARIANT CATEGORIES:
//
// 1. PRICE INVARIANTS - Price bounds, tick-price consistency
// 2. LIQUIDITY INVARIANTS - Conservation, bounds, delta safety
// 3. SWAP INVARIANTS - Value conservation, direction, limits
// 4. FEE INVARIANTS - Monotonicity, bounds
// 5. TICK INVARIANTS - Bitmap consistency, spacing
// 6. MATH INVARIANTS - Overflow safety, rounding correctness
//
// USAGE:
// - Unit tests validate invariants for concrete test cases (cargo test)
// - Full formal verification requires Certora Sunbeam prover
//
// NOTE: Full Certora Sunbeam integration is pending package updates for
// soroban-sdk 23.0.0 compatibility. See:
// - https://github.com/Certora/solana-cvt (dev-soroban branch)
// - https://docs.certora.com/en/latest/docs/sunbeam/installation.html
//
// ============================================================================

// Spec modules with unit tests validating invariants
pub mod math_specs;
pub mod pool_state_specs;
pub mod swap_specs;
pub mod liquidity_specs;
pub mod tick_specs;
