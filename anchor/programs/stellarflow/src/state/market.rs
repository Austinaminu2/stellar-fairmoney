use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Market {
    /// Authority that can manage the market
    pub authority: Pubkey,
    /// Whether the market is paused
    pub is_paused: bool,
    /// Number of reserves in this market
    pub reserve_count: u64,
    /// Protocol fee in basis points (e.g. 50 = 0.5%)
    pub protocol_fee_bps: u64,
    /// Treasury to collect fees
    pub treasury: Pubkey,
    /// Bump seed
    pub bump: u8,
}
