use anchor_lang::prelude::*;
use crate::state::Market;

pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    protocol_fee_bps: u64,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    market.authority = ctx.accounts.authority.key();
    market.treasury = ctx.accounts.treasury.key();
    market.is_paused = false;
    market.reserve_count = 0;
    market.protocol_fee_bps = protocol_fee_bps;
    market.bump = ctx.bumps.market;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeMarket<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Market::INIT_SPACE,
        seeds = [b"market", authority.key().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Just storing the public key
    pub treasury: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
