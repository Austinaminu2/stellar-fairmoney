use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::state::{Market, Reserve, UserPosition};
use crate::errors::FairMoneyError;

pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    require!(amount > 0, FairMoneyError::ZeroAmount);
    require!(!ctx.accounts.market.is_paused, FairMoneyError::MarketPaused);

    let position = &ctx.accounts.user_position;
    require!(
        position.deposited_amount >= amount,
        FairMoneyError::InsufficientDeposit
    );

    let reserve = &ctx.accounts.reserve;
    require!(
        reserve.total_deposits.saturating_sub(reserve.total_borrows) >= amount,
        FairMoneyError::InsufficientLiquidity
    );

    let clock = Clock::get()?;
    let market_key = ctx.accounts.market.key();
    let token_mint_key = ctx.accounts.reserve.token_mint;
    let reserve_bump = ctx.accounts.reserve.bump;

    // PDA signer seeds for reserve vault authority
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
    reserve.total_deposits = reserve.total_deposits.checked_sub(amount)
        .ok_or(FairMoneyError::MathOverflow)?;
    reserve.last_update_timestamp = clock.unix_timestamp;

    // Update position
    let position = &mut ctx.accounts.user_position;
    position.deposited_amount = position.deposited_amount.checked_sub(amount)
        .ok_or(FairMoneyError::MathOverflow)?;
    position.last_update_timestamp = clock.unix_timestamp;

    emit!(WithdrawEvent {
        user: ctx.accounts.user.key(),
        reserve: ctx.accounts.reserve.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub reserve: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
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
        has_one = owner,
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub liquidity_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    /// CHECK: Verified via has_one on user_position
    pub owner: AccountInfo<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
