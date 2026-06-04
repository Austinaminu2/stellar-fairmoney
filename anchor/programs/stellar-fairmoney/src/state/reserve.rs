use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Reserve {
    /// The market this reserve belongs to
    pub market: Pubkey,
    /// The token mint for this reserve
    pub token_mint: Pubkey,
    /// The reserve's token account holding liquidity
    pub liquidity_vault: Pubkey,
    /// Total liquidity deposited
    pub total_deposits: u64,
    /// Total amount borrowed
    pub total_borrows: u64,
    /// Cumulative borrow rate (scaled by 1e9)
    pub cumulative_borrow_rate: u64,
    /// Last time interest was accrued (unix timestamp)
    pub last_update_timestamp: i64,
    /// Base borrow rate per second (scaled by 1e9)
    pub base_borrow_rate: u64,
    /// Optimal utilization rate in basis points
    pub optimal_utilization_bps: u64,
    /// Borrow rate slope below optimal utilization (scaled by 1e9)
    pub slope1: u64,
    /// Borrow rate slope above optimal utilization (scaled by 1e9)
    pub slope2: u64,
    /// Loan-to-value ratio in basis points (e.g. 7500 = 75%)
    pub ltv_bps: u64,
    /// Liquidation threshold in basis points (e.g. 8000 = 80%)
    pub liquidation_threshold_bps: u64,
    /// Liquidation bonus in basis points (e.g. 500 = 5%)
    pub liquidation_bonus_bps: u64,
    /// Mock price in USD (scaled by 1e6, e.g. 1_000_000 = $1.00)
    pub mock_price: u64,
    /// Whether this reserve is active
    pub is_active: bool,
    /// Bump seed
    pub bump: u8,
}

impl Reserve {
    /// Calculate current utilization rate in basis points
    pub fn utilization_rate_bps(&self) -> u64 {
        if self.total_deposits == 0 {
            return 0;
        }
        (self.total_borrows * 10_000) / self.total_deposits
    }

    /// Calculate current borrow APR in basis points using two-slope model
    pub fn borrow_rate_bps(&self) -> u64 {
        let utilization = self.utilization_rate_bps();
        if utilization <= self.optimal_utilization_bps {
            // Below optimal: base + slope1 * (utilization / optimal)
            let slope = (self.slope1 * utilization) / self.optimal_utilization_bps.max(1);
            self.base_borrow_rate + slope
        } else {
            // Above optimal: base + slope1 + slope2 * ((util - optimal) / (1 - optimal))
            let excess = utilization - self.optimal_utilization_bps;
            let denominator = (10_000u64).saturating_sub(self.optimal_utilization_bps).max(1);
            let slope = (self.slope2 * excess) / denominator;
            self.base_borrow_rate + self.slope1 + slope
        }
    }

    /// Calculate supply APR in basis points
    pub fn supply_rate_bps(&self) -> u64 {
        let utilization = self.utilization_rate_bps();
        let borrow_rate = self.borrow_rate_bps();
        (borrow_rate * utilization) / 10_000
    }
}
