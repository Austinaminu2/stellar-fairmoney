use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::FairMoneyError;

pub fn liquidate(ctx: Context<Liquidate>, repay_amount: u64) -> Result<()> {
    require!(repay_amount > 0, FairMoneyError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, FairMoneyError::MarketPaused);

    let clock = Clock::get()?;

    let collateral_deposited = ctx.accounts.collateral_position.deposited_amount;
    let collateral_price = ctx.accounts.collateral_reserve.mock_price;
    let collateral_lthr = ctx.accounts.collateral_reserve.liquidation_threshold_bps;
    let collateral_bonus = ctx.accounts.collateral_reserve.liquidation_bonus_bps;
    let collateral_mint_key = ctx.accounts.collateral_reserve.token_mint;
    let collateral_bump = ctx.accounts.collateral_reserve.bump;

    let borrow_price = ctx.accounts.borrow_reserve.mock_price;
    let borrow_rate = ctx.accounts.borrow_reserve.borrow_rate_bps();

    let borrowed = ctx.accounts.borrow_position.borrowed_amount;
    let last_ts = ctx.accounts.borrow_position.last_update_timestamp;

    let collateral_value_usd = collateral_deposited
        .checked_mul(collateral_price)
        .ok_or(FairMoneyError::MathOverflow)?
        / 1_000_000;

    let time_elapsed = (clock.unix_timestamp - last_ts).max(0) as u64;
    let seconds_per_year: u64 = 31_536_000;
    let accrued_interest = borrowed
        .saturating_mul(borrow_rate)
        .saturating_mul(time_elapsed)
        / (seconds_per_year * 10_000);

    let total_borrowed = borrowed.saturating_add(accrued_interest);

    let borrow_value_usd = total_borrowed
        .checked_mul(borrow_price)
        .ok_or(FairMoneyError::MathOverflow)?
        / 1_000_000;

    let liquidation_threshold_value = collateral_value_usd
        .checked_mul(collateral_lthr)
        .ok_or(FairMoneyError::MathOverflow)?
        / 10_000;

    require!(
        borrow_value_usd > liquidation_threshold_value,
        FairMoneyError::PositionHealthy
    );

    let max_repay = total_borrowed / 2;
    require!(repay_amount <= max_repay, FairMoneyError::LiquidationAmountTooLarge);

    let repay_value_usd = repay_amount
        .checked_mul(borrow_price)
        .ok_or(FairMoneyError::MathOverflow)?
        / 1_000_000;

    let collateral_to_seize_usd = repay_value_usd
        .checked_mul(10_000 + collateral_bonus)
        .ok_or(FairMoneyError::MathOverflow)?
        / 10_000;

    let collateral_to_seize = collateral_to_seize_usd
        .checked_mul(1_000_000)
        .ok_or(FairMoneyError::MathOverflow)?
        / collateral_price.max(1);

    let collateral_to_seize = collateral_to_seize.min(collateral_deposited);

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.liquidator_repay_account.to_account_info(),
                to: ctx.accounts.borrow_vault.to_account_info(),
                authority: ctx.accounts.liquidator.to_account_info(),
            },
        ),
        repay_amount,
    )?;

    let market_key = ctx.accounts.market.key();
    let seeds = &[
        b"reserve",
        market_key.as_ref(),
        collateral_mint_key.as_ref(),
        &[collateral_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.collateral_vault.to_account_info(),
                to: ctx.accounts.liquidator_collateral_account.to_account_info(),
                authority: ctx.accounts.collateral_reserve.to_account_info(),
            },
            signer_seeds,
        ),
        collateral_to_seize,
    )?;

    ctx.accounts.borrow_position.borrowed_amount = borrowed.saturating_sub(repay_amount);
    ctx.accounts.borrow_position.last_update_timestamp = clock.unix_timestamp;
    ctx.accounts.collateral_position.deposited_amount = collateral_deposited.saturating_sub(collateral_to_seize);
    ctx.accounts.collateral_position.last_update_timestamp = clock.unix_timestamp;
    ctx.accounts.borrow_reserve.total_borrows = ctx.accounts.borrow_reserve.total_borrows.saturating_sub(repay_amount);
    ctx.accounts.collateral_reserve.total_deposits = ctx.accounts.collateral_reserve.total_deposits.saturating_sub(collateral_to_seize);

    emit!(LiquidateEvent {
        liquidator: ctx.accounts.liquidator.key(),
        borrower: ctx.accounts.borrower.key(),
        repay_amount,
        collateral_seized: collateral_to_seize,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct LiquidateEvent {
    pub liquidator: Pubkey,
    pub borrower: Pubkey,
    pub repay_amount: u64,
    pub collateral_seized: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(
        seeds = [b"market", market.authority.as_ref()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"reserve", market.key().as_ref(), borrow_reserve.token_mint.as_ref()],
        bump = borrow_reserve.bump,
    )]
    pub borrow_reserve: Account<'info, Reserve>,

    #[account(
        mut,
        seeds = [b"reserve", market.key().as_ref(), collateral_reserve.token_mint.as_ref()],
        bump = collateral_reserve.bump,
    )]
    pub collateral_reserve: Account<'info, Reserve>,

    #[account(
        mut,
        seeds = [b"position", borrow_reserve.key().as_ref(), borrower.key().as_ref()],
        bump = borrow_position.bump,
    )]
    pub borrow_position: Account<'info, UserPosition>,

    #[account(
        mut,
        seeds = [b"position", collateral_reserve.key().as_ref(), borrower.key().as_ref()],
        bump = collateral_position.bump,
    )]
    pub collateral_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub borrow_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub collateral_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub liquidator_repay_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub liquidator_collateral_account: Account<'info, TokenAccount>,

    /// CHECK: The borrower being liquidated
    pub borrower: AccountInfo<'info>,

    pub borrow_mint: Account<'info, Mint>,
    pub collateral_mint: Account<'info, Mint>,

    #[account(mut)]
    pub liquidator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
