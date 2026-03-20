use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("GdGbp1YQJxSwjTU9sa5JDnDwLoU3tbtgwLT7UyEJ4rWd");

#[program]
pub mod stellar_flow {
    use super::*;

    /// Initialize a new lending market
    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        protocol_fee_bps: u64,
    ) -> Result<()> {
        instructions::initialize_market::initialize_market(ctx, protocol_fee_bps)
    }

    /// Initialize a new reserve (token market) within the lending market
    pub fn initialize_reserve(
        ctx: Context<InitializeReserve>,
        ltv_bps: u64,
        liquidation_threshold_bps: u64,
        liquidation_bonus_bps: u64,
        optimal_utilization_bps: u64,
        base_borrow_rate: u64,
        slope1: u64,
        slope2: u64,
        mock_price: u64,
    ) -> Result<()> {
        instructions::initialize_reserve::initialize_reserve(
            ctx,
            ltv_bps,
            liquidation_threshold_bps,
            liquidation_bonus_bps,
            optimal_utilization_bps,
            base_borrow_rate,
            slope1,
            slope2,
            mock_price,
        )
    }

    /// Deposit tokens into a reserve to earn interest
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::deposit(ctx, amount)
    }

    /// Withdraw previously deposited tokens
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        instructions::withdraw::withdraw(ctx, amount)
    }

    /// Borrow tokens against deposited collateral
    pub fn borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
        instructions::borrow::borrow(ctx, amount)
    }

    /// Repay borrowed tokens
    pub fn repay(ctx: Context<Repay>, amount: u64) -> Result<()> {
        instructions::repay::repay(ctx, amount)
    }

    /// Liquidate an undercollateralized position
    pub fn liquidate(ctx: Context<Liquidate>, repay_amount: u64) -> Result<()> {
        instructions::liquidate::liquidate(ctx, repay_amount)
    }
}
