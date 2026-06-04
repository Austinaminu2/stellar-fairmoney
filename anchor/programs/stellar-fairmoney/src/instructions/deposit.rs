use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::FairMoneyError;

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, FairMoneyError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, FairMoneyError::MarketPaused);
    require!(ctx.accounts.reserve.is_active, FairMoneyError::ReserveNotActive);

    let clock = Clock::get()?;

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
        amount,
    )?;

    // Update reserve
    let reserve = &mut ctx.accounts.reserve;
    reserve.total_deposits = reserve.total_deposits.checked_add(amount)
        .ok_or(FairMoneyError::MathOverflow)?;
    reserve.last_update_timestamp = clock.unix_timestamp;

    // Update user position
    let position = &mut ctx.accounts.user_position;
    position.owner = ctx.accounts.user.key();
    position.reserve = ctx.accounts.reserve.key();
    position.deposited_amount = position.deposited_amount.checked_add(amount)
        .ok_or(FairMoneyError::MathOverflow)?;
    position.last_update_timestamp = clock.unix_timestamp;

    emit!(DepositEvent {
        user: ctx.accounts.user.key(),
        reserve: ctx.accounts.reserve.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub reserve: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
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
        init_if_needed,
        payer = user,
        space = 8 + UserPosition::INIT_SPACE,
        seeds = [b"position", reserve.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub liquidity_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// CHECK: Mint is verified via token account
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
