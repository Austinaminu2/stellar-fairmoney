use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserPosition {
    /// The user who owns this position
    pub owner: Pubkey,
    /// The reserve this position is for
    pub reserve: Pubkey,
    /// Amount deposited (in token base units)
    pub deposited_amount: u64,
    /// Amount borrowed (in token base units)
    pub borrowed_amount: u64,
    /// Borrow rate at time of last borrow update (scaled by 1e9)
    pub last_borrow_rate: u64,
    /// Timestamp of last update
    pub last_update_timestamp: i64,
    /// Bump seed
    pub bump: u8,
}

impl UserPosition {
    /// Calculate accrued interest on borrowed amount
    pub fn accrued_borrow_interest(&self, current_rate: u64, current_timestamp: i64) -> u64 {
        if self.borrowed_amount == 0 {
            return 0;
        }
        let time_elapsed = (current_timestamp - self.last_update_timestamp).max(0) as u64;
        // Simple interest: principal * rate * time / (seconds_per_year * scale)
        let seconds_per_year: u64 = 31_536_000;
        let interest = self.borrowed_amount
            .saturating_mul(current_rate)
            .saturating_mul(time_elapsed)
            / (seconds_per_year * 10_000);
        interest
    }
}
