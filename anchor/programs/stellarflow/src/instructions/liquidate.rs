use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::StellarFlowError;

pub fn liquidate(ctx: Context<Liquidate>, repay_amount: u64) -> Result<()> {
    require!(repay_amount > 0, StellarFlowError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, StellarFlowError::MarketPaused);

    let clock = Clock::get()?;
    let borrow_reserve = &ctx.accounts.borrow_reserve;
    let collateral_reserve = &ctx.accounts.collateral_reserve;
    let borrow_position = &ctx.accounts.borrow_position;
    let collateral_position = &ctx.accounts.collateral_position;

    // Calculate collateral value in USD
    let collateral_value_usd = collateral_position.deposited_amount
        .checked_mul(collateral_reserve.mock_price)
        .ok_or(StellarFlowError::MathOverflow)?
        / 1_000_000;

    // Calculate borrow value in USD including interest
    let accrued_interest = borrow_position.accrued_borrow_interest(
        borrow_reserve.borrow_rate_bps(),
        clock.unix_timestamp,
    );
    let total_borrowed = borrow_position.borrowed_amount.saturating_add(accrued_interest);
    let borrow_value_usd = total_borrowed
        .checked_mul(borrow_reserve.mock_price)
        .ok_or(StellarFlowError::MathOverflow)?
        / 1_000_000;

    // Check if position is undercollateralized
    let liquidation_threshold_value = collateral_value_usd
        .checked_mul(collateral_reserve.liquidation_threshold_bps)
        .ok_or(StellarFlowError::MathOverflow)?
        / 10_000;

    require!(
        borrow_value_usd > liquidation_threshold_value,
        StellarFlowError::PositionHealthy
    );

    // Max repay is 50% of the borrowed amount
    let max_repay = total_borrowed / 2;
    require!(repay_amount <= max_repay, StellarFlowError::LiquidationAmountTooLarge);

    // Calculate collateral to seize including bonus
    let repay_value_usd = repay_amount
        .checked_mul(borrow_reserve.mock_price)
        .ok_or(StellarFlowError::MathOverflow)?
        / 1_000_000;

    let collateral_to_seize_usd = repay_value_usd
        .checked_mul(10_000 + collateral_reserve.liquidation_bonus_bps)
        .ok_or(StellarFlowError::MathOverflow)?
        / 10_000;

    let collateral_to_seize = collateral_to_seize_usd
        .checked_mul(1_000_000)
        .ok_or(StellarFlowError::MathOverflow)?
        / collateral_reserve.mock_price.max(1);

    let collateral_to_seize = collateral_to_seize.min(collateral_position.deposited_amount);

    // Liquidator repays the borrow
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

    // Liquidator receives collateral
    let market_key = ctx.accounts.market.key();
    let collateral_mint_key = collateral_reserve.token_mint;
    let collateral_bump = collateral_reserve.bump;

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

    // Update borrow position
    let borrow_position = &mut ctx.accounts.borrow_position;
    borrow_position.borrowed_amount = borrow_position.borrowed_amount.saturating_sub(repay_amount);
    borrow_position.last_update_timestamp = clock.unix_timestamp;

    // Update collateral position
    let collateral_position = &mut ctx.accounts.collateral_position;
    collateral_position.deposited_amount = collateral_position.deposited_amount
        .saturating_sub(collateral_to_seize);
    collateral_position.last_update_timestamp = clock.unix_timestamp;

    // Update reserves
    let borrow_reserve = &mut ctx.accounts.borrow_reserve;
    borrow_reserve.total_borrows = borrow_reserve.total_borrows.saturating_sub(repay_amount);

    let collateral_reserve = &mut ctx.accounts.collateral_reserve;
    collateral_reserve.total_deposits = collateral_reserve.total_deposits
        .saturating_sub(collateral_to_seize);

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
        has_one = borrow_vault @ StellarFlowError::InsufficientLiquidity,
    )]
    pub borrow_reserve: Account<'info, Reserve>,

    #[account(
        mut,
        seeds = [b"reserve", market.key().as_ref(), collateral_reserve.token_mint.as_ref()],
        bump = collateral_reserve.bump,
        has_one = collateral_vault @ StellarFlowError::InsufficientLiquidity,
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
