use crate::storage::{ReserveConfig, ReserveData};

const INDEX_SCALE: i128 = 1_000_000_000; // 1e9
/// ~5 ledgers/sec → 1 year ≈ 157_680_000 ledgers
const LEDGERS_PER_YEAR: i128 = 157_680_000;

/// Two-slope borrow rate in bps
pub fn borrow_rate_bps(config: &ReserveConfig, data: &ReserveData) -> u32 {
    let utilization = if data.total_deposits == 0 {
        0u32
    } else {
        ((data.total_borrows * 10_000) / data.total_deposits) as u32
    };

    if utilization <= config.optimal_utilization_bps {
        let slope = config.slope1_bps as u64 * utilization as u64
            / config.optimal_utilization_bps.max(1) as u64;
        config.base_rate_bps + slope as u32
    } else {
        let excess = utilization - config.optimal_utilization_bps;
        let denom = (10_000u32)
            .saturating_sub(config.optimal_utilization_bps)
            .max(1);
        let slope = config.slope2_bps as u64 * excess as u64 / denom as u64;
        config.base_rate_bps + config.slope1_bps + slope as u32
    }
}

/// Compound the borrow index over elapsed ledgers.
/// new_index = old_index * (1 + rate * time)
/// Returns (new_index, interest_accrued_on_total_borrows)
pub fn accrue_index(data: &ReserveData, config: &ReserveConfig, current_ledger: u32) -> (i128, i128) {
    let elapsed = current_ledger.saturating_sub(data.last_update_ledger) as i128;
    if elapsed == 0 {
        return (data.borrow_index, 0);
    }
    let rate = borrow_rate_bps(config, data) as i128;
    // interest_factor = rate * elapsed / (LEDGERS_PER_YEAR * 10_000)
    // new_index = old_index + old_index * interest_factor
    let interest_factor = rate * elapsed;
    let index_delta = data.borrow_index * interest_factor / (LEDGERS_PER_YEAR * 10_000);
    let new_index = data.borrow_index + index_delta;

    // interest on total borrows
    let interest = data.total_borrows * interest_factor / (LEDGERS_PER_YEAR * 10_000);
    (new_index, interest)
}

/// Convert borrow shares back to token amount using current index
pub fn shares_to_amount(shares: i128, index: i128) -> i128 {
    shares * index / INDEX_SCALE
}

/// Convert token amount to borrow shares using current index
pub fn amount_to_shares(amount: i128, index: i128) -> i128 {
    amount * INDEX_SCALE / index
}
