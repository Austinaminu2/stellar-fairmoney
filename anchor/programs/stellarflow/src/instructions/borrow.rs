use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::StellarFlowError;

pub fn borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
    require!(amount > 0, StellarFlowError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, StellarFlowError::MarketPaused);
    require!(ctx.accounts.reserve.is_active, StellarFlowError::ReserveNotActive);

    let reserve = &ctx.accounts.reserve;
    let collateral_position = &ctx.accounts.collateral_position;

    // Calculate collateral value in USD (scaled by 1e6)
    let collateral_value = collateral_position.deposited_amount
        .checked_mul(ctx.accounts.collateral_reserve.mock_price)
        .ok_or(StellarFlowError::MathOverflow)?
        / 1_000_000;

    // Maximum borrow value based on LTV
    let max_borrow_value = collateral_value
        .checked_mul(ctx.accounts.collateral_reserve.ltv_bps)
        .ok_or(StellarFlowError::MathOverflow)?
        / 10_000;

    // Current borrow value including this new borrow
    let borrow_value = (collateral_position.borrowed_amount + amount)
        .checked_mul(reserve.mock_price)
        .ok_or(StellarFlowError::MathOverflow)?
        / 1_000_000;

    require!(borrow_value <= max_borrow_value, StellarFlowError::InsufficientCollateral);

    // Check liquidity
    let available_liquidity = reserve.total_deposits.saturating_sub(reserve.total_borrows);
    require!(available_liquidity >= amount, StellarFlowError::InsufficientLiquidity);

    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let token_mint_key = ctx.accounts.reserve.token_mint;
    let reserve_bump = ctx.accounts.reserve.bump;

    let seeds = &[
        b"reserve",
        market_key.as_ref(),
        token_mint_key.as_ref(),
        &[reserve_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    // Transfer tokens from vault to user
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.liquidity_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.reserve.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    // Update reserve
    let reserve = &mut ctx.accounts.reserve;
    reserve.total_borrows = reserve.total_borrows.checked_add(amount)
        .ok_or(StellarFlowError::MathOverflow)?;
    reserve.last_update_timestamp = clock.unix_timestamp;

    // Update user borrow position
    let borrow_position = &mut ctx.accounts.borrow_position;
    borrow_position.owner = ctx.accounts.user.key();
    borrow_position.reserve = ctx.accounts.reserve.key();
    borrow_position.borrowed_amount = borrow_position.borrowed_amount.checked_add(amount)
        .ok_or(StellarFlowError::MathOverflow)?;
    borrow_position.last_borrow_rate = reserve.borrow_rate_bps();
    borrow_position.last_update_timestamp = clock.unix_timestamp;

    emit!(BorrowEvent {
        user: ctx.accounts.user.key(),
        reserve: ctx.accounts.reserve.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct BorrowEvent {
    pub user: Pubkey,
    pub reserve: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct Borrow<'info> {
    #[account(
        seeds = [b"market", market.authority.as_ref()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    /// The reserve being borrowed from
    #[account(
        mut,
        seeds = [b"reserve", market.key().as_ref(), reserve.token_mint.as_ref()],
        bump = reserve.bump,
        has_one = liquidity_vault,
    )]
    pub reserve: Account<'info, Reserve>,

    /// The reserve used as collateral
    #[account(
        seeds = [b"reserve", market.key().as_ref(), collateral_reserve.token_mint.as_ref()],
        bump = collateral_reserve.bump,
    )]
    pub collateral_reserve: Account<'info, Reserve>,

    /// User's collateral position
    #[account(
        seeds = [b"position", collateral_reserve.key().as_ref(), user.key().as_ref()],
        bump = collateral_position.bump,
    )]
    pub collateral_position: Account<'info, UserPosition>,

    /// User's borrow position for this reserve
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + UserPosition::INIT_SPACE,
        seeds = [b"position", reserve.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub borrow_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub liquidity_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
