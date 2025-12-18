# Stellar DEX v3

A Uniswap v3-style concentrated liquidity DEX implementation on Stellar Soroban.

## Overview

This project implements the core mechanics of Uniswap v3's concentrated liquidity automated market maker (AMM) on the Stellar blockchain using Soroban smart contracts. It enables liquidity providers to concentrate their capital within custom price ranges, dramatically improving capital efficiency compared to traditional constant-product AMMs.

### Key Features

- **Concentrated Liquidity**: LPs can provide liquidity within specific price ranges
- **Multiple Fee Tiers**: Support for 0.05%, 0.3%, and 1% fee tiers (configurable)
- **Capital Efficiency**: Up to 4000x more efficient than v2-style AMMs
- **NFT-like Positions**: Each LP position is uniquely tracked with its own fee accounting
- **Multi-hop Routing**: Swap through multiple pools in a single transaction

### Scope

This implementation includes the core concentrated liquidity mechanics. The following features are **not** included:
- Oracle/TWAP functionality
- Flash swaps
- Protocol fee collection (infrastructure exists but not activated)

## Project Structure

```
stellar-dex-v3/
├── contracts/
│   ├── dex-factory/           # Pool deployment & registry
│   ├── dex-pool/              # Core AMM logic (swaps, liquidity)
│   ├── dex-router/            # Multi-hop swaps, deadline protection
│   ├── dex-quoter/            # Off-chain price quotes
│   └── dex-position-manager/  # NFT-like LP position tracking
├── libs/
│   ├── dex-math/              # Fixed-point math library
│   │   ├── full_math.rs       # mul_div with 256-bit intermediate
│   │   ├── tick_math.rs       # Tick <-> sqrt price conversions
│   │   ├── sqrt_price_math.rs # Price and amount calculations
│   │   ├── liquidity_math.rs  # Liquidity calculations
│   │   └── swap_math.rs       # Swap step computation
│   └── dex-types/             # Shared types and constants
└── Cargo.toml                 # Workspace configuration
```

## Implementation Highlights

### Concentrated Liquidity Math

The core innovation is representing prices as `sqrt(price)` in Q64.96 fixed-point format (`u128`). This enables:

- Efficient tick-to-price conversions using precomputed constants
- Precise liquidity and amount calculations without overflow
- Constant-product invariant maintenance within each tick range

```rust
// Price is stored as sqrt(price) * 2^96
pub type SqrtPriceX96 = u128;

// Each tick represents a 0.01% price change
// tick_spacing determines which ticks can be initialized
pub const MIN_TICK: i32 = -887272;
pub const MAX_TICK: i32 = 887272;
```

### Tick Bitmap

Efficiently tracks which ticks have liquidity using a bitmap structure:

- **128 ticks per word** (using `u128` native type)
- O(1) lookup for next initialized tick within a word
- Automatic cleanup when ticks become empty

### Fee Tiers

| Fee | Tick Spacing | Use Case |
|-----|-------------|----------|
| 0.05% (500) | 10 | Stable pairs |
| 0.3% (3000) | 60 | Standard pairs |
| 1% (10000) | 200 | Exotic pairs |

### Position Management

Each LP position tracks:
- Tick range (`tick_lower`, `tick_upper`)
- Liquidity amount
- Fee growth checkpoints (for accurate fee accounting)
- Tokens owed (accumulated fees + withdrawn liquidity)

## Adaptations from Uniswap v3

| Uniswap v3 (Ethereum) | Stellar Soroban |
|----------------------|-----------------|
| Solidity | Rust (no_std) |
| ERC-20 tokens | SEP-41 token interface |
| ERC-721 NFT positions | Custom position tracking with NFT-like ownership |
| 256-bit integers (uint256) | Native `U256`/`I256` from soroban-sdk |
| Tick bitmap with uint256 words | `u128` words (128 ticks per word) |
| Reentrancy guards | Not needed (Soroban prevents reentrancy at protocol level) |
| CREATE2 deployment | `env.deployer().with_current_contract(salt)` |
| Block.timestamp | `env.ledger().timestamp()` |
| Immutable storage | Instance storage with TTL extension |

### Key Soroban-Specific Adaptations

#### Storage Architecture
Soroban has different storage semantics than Ethereum:
- **Instance Storage**: Contract-wide data (config, state) - extends TTL together
- **Persistent Storage**: Per-key data (ticks, positions) - individual TTL management
- **Automatic cleanup**: Empty entries are removed to save storage costs

#### Resource Limits
Designed to stay within Soroban's transaction limits:

| Resource | Limit | Design Impact |
|----------|-------|---------------|
| Ledger entry size | 128 KiB | Indexed storage instead of unbounded Vec |
| Read entries/tx | 100 | Pagination for large queries |
| Write entries/tx | 50 | Max 40 tick crossings per swap |
| Storage key size | 250 bytes | Compact key structures |

#### Fixed-Point Math
Uses Soroban's native `U256` for intermediate calculations:

```rust
// Multiply and divide with 256-bit intermediate precision
pub fn mul_div(env: &Env, a: u128, b: u128, denominator: u128) -> u128 {
    let a_256 = U256::from_u128(env, a);
    let b_256 = U256::from_u128(env, b);
    let denom_256 = U256::from_u128(env, denominator);

    let product = a_256.mul(&b_256);
    let result = product.div(&denom_256);

    u128_from_u256(env, &result)
}
```

## Contract Interfaces

### Factory
```rust
fn initialize(env, admin, pool_wasm_hash)
fn create_pool(env, token_a, token_b, fee, initial_sqrt_price_x96) -> Address
fn get_pool(env, token_a, token_b, fee) -> Option<Address>
fn enable_fee_amount(env, fee, tick_spacing)
```

### Pool
```rust
fn swap(env, recipient, zero_for_one, amount_specified, sqrt_price_limit_x96) -> (i128, i128)
fn mint(env, recipient, tick_lower, tick_upper, amount) -> (u128, u128)
fn burn(env, tick_lower, tick_upper, amount) -> (u128, u128)
fn collect(env, recipient, tick_lower, tick_upper, amount0_max, amount1_max) -> (u128, u128)
```

### Position Manager
```rust
fn mint(env, params: MintParams) -> (u32, u128, i128, i128)
fn increase_liquidity(env, params: IncreaseLiquidityParams) -> (u128, i128, i128)
fn decrease_liquidity(env, params: DecreaseLiquidityParams) -> (i128, i128)
fn collect(env, params: CollectParams) -> (u128, u128)
fn burn(env, position_id: u32)
```

### Router
```rust
fn exact_input_single(env, params: ExactInputSingleParams) -> i128
fn exact_output_single(env, params: ExactOutputSingleParams) -> i128
fn exact_input(env, params: ExactInputParams) -> i128
fn exact_output(env, params: ExactOutputParams) -> i128
```

## Building

```bash
# Build all contracts
cargo build --release --target wasm32-unknown-unknown

# Run tests
cargo test

# Build optimized WASM (requires soroban-cli)
soroban contract build
```

## Testing

The project includes comprehensive unit tests covering:

- **Math Libraries** (100 tests): tick math, sqrt price math, liquidity math, swap math
- **Pool Contract** (34 tests): initialization, tick management, fee growth
- **Factory Contract** (17 tests): pool creation, fee tier management

```bash
# Run all tests
cargo test

# Run specific test module
cargo test --package dex-math
cargo test --package dex-pool
```

## Dependencies

```toml
[workspace.dependencies]
soroban-sdk = "23.0.0"
soroban-fixed-point-math = "1.3.0"
```

## Architecture Decisions

### Why Concentrated Liquidity?
Traditional AMMs spread liquidity across the entire price curve (0 to infinity). Concentrated liquidity allows LPs to focus their capital where trading actually occurs, earning more fees with less capital.

### Why Q64.96 Fixed-Point?
- Matches Uniswap v3's format for easy constant reuse
- Sufficient precision for price representation
- Fits in `u128` for efficient Soroban operations

### Why Separate Contracts?
- **Modularity**: Each contract has a single responsibility
- **Upgradeability**: Pool logic can be upgraded via factory
- **Gas efficiency**: Users only load contracts they need

## Security Considerations

- **No Reentrancy**: Soroban's execution model prevents reentrancy attacks
- **Overflow Protection**: All math uses checked operations or explicit overflow handling
- **Price Bounds**: Enforced minimum and maximum sqrt price ratios
- **Slippage Protection**: All swap/liquidity functions support minimum amount checks
- **Deadline Protection**: Router functions include transaction deadline validation

## License

MIT

## Acknowledgments

This implementation is based on [Uniswap v3](https://github.com/Uniswap/v3-core) by Uniswap Labs. The concentrated liquidity mechanism was pioneered by the Uniswap team.
