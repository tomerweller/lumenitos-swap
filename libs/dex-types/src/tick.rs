use soroban_sdk::contracttype;

/// Information stored for each initialized tick
#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct TickInfo {
    /// Total liquidity referencing this tick
    pub liquidity_gross: u128,
    /// Net liquidity change when tick is crossed (+ when moving right)
    pub liquidity_net: i128,
    /// Fee growth per unit liquidity on token0 side when tick was last crossed
    pub fee_growth_outside_0_x128: u128,
    /// Fee growth per unit liquidity on token1 side when tick was last crossed
    pub fee_growth_outside_1_x128: u128,
    /// True if tick has been initialized
    pub initialized: bool,
}

impl TickInfo {
    pub fn new() -> Self {
        Self::default()
    }
}
