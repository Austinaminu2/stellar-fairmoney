use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::state::{Market, Reserve};
use crate::errors::FairMoneyError;

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
    require!(!ctx.accounts.market.is_paused, FairMoneyError::MarketPaused);

    let clock = Clock::get()?;
    let reserve = &mut ctx.accounts.reserve;

    reserve.market = ctx.accounts.market.key();
    reserve.token_mint = ctx.accounts.token_mint.key();
    reserve.liquidity_vault = ctx.accounts.liquidity_vault.key();
    reserve.total_deposits = 0;
    reserve.total_borrows = 0;
    reserve.cumulative_borrow_rate = 1_000_000_000; // 1.0 scaled by 1e9
    reserve.last_update_timestamp = clock.unix_timestamp;
    reserve.base_borrow_rate = base_borrow_rate;
    reserve.optimal_utilization_bps = optimal_utilization_bps;
    reserve.slope1 = slope1;
    reserve.slope2 = slope2;
    reserve.ltv_bps = ltv_bps;
    reserve.liquidation_threshold_bps = liquidation_threshold_bps;
    reserve.liquidation_bonus_bps = liquidation_bonus_bps;
    reserve.mock_price = mock_price;
    reserve.is_active = true;
    reserve.bump = ctx.bumps.reserve;

    let market = &mut ctx.accounts.market;
    market.reserve_count += 1;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeReserve<'info> {
    #[account(
        mut,
        seeds = [b"market", market.authority.as_ref()],
        bump = market.bump,
        has_one = authority
    )]
    pub market: Account<'info, Market>,

    #[account(
        init,
        payer = authority,
        space = 8 + Reserve::INIT_SPACE,
        seeds = [b"reserve", market.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub reserve: Account<'info, Reserve>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = token_mint,
        token::authority = reserve,
        seeds = [b"vault", reserve.key().as_ref()],
        bump
    )]
    pub liquidity_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
