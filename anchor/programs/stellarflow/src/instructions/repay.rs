use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::StellarFlowError;

pub fn repay(ctx: Context<Repay>, amount: u64) -> Result<()> {
    require!(amount > 0, StellarFlowError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, StellarFlowError::MarketPaused);

    let position = &ctx.accounts.user_position;
    let clock = Clock::get()?;

    // Calculate total owed including accrued interest
    let accrued_interest = position.accrued_borrow_interest(
        ctx.accounts.reserve.borrow_rate_bps(),
        clock.unix_timestamp,
    );
    let total_owed = position.borrowed_amount.saturating_add(accrued_interest);
    let repay_amount = amount.min(total_owed);

    require!(repay_amount > 0, StellarFlowError::InsufficientBorrow);

    // Transfer tokens from user to vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.liquidity_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        repay_amount,
    )?;

    // Update reserve
    let reserve = &mut ctx.accounts.reserve;
    reserve.total_borrows = reserve.total_borrows.saturating_sub(repay_amount);
    reserve.last_update_timestamp = clock.unix_timestamp;

    // Update position
    let position = &mut ctx.accounts.user_position;
    position.borrowed_amount = position.borrowed_amount.saturating_sub(repay_amount);
    position.last_update_timestamp = clock.unix_timestamp;

    emit!(RepayEvent {
        user: ctx.accounts.user.key(),
        reserve: ctx.accounts.reserve.key(),
        amount: repay_amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct RepayEvent {
    pub user: Pubkey,
    pub reserve: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(
        seeds = [b"market", market.authority.as_ref()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"reserve", market.key().as_ref(), reserve.token_mint.as_ref()],
        bump = reserve.bump,
        has_one = liquidity_vault,
    )]
    pub reserve: Account<'info, Reserve>,

    #[account(
        mut,
        seeds = [b"position", reserve.key().as_ref(), user.key().as_ref()],
        bump = user_position.bump,
    )]
    pub user_position: Account<'info, UserPosition>,

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
