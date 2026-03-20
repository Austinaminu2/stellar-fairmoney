use anchor_lang::prelude::*;

#[error_code]
pub enum StellarFlowError {
    #[msg("Market is paused")]
    MarketPaused,
    #[msg("Reserve is not active")]
    ReserveNotActive,
    #[msg("Insufficient liquidity in reserve")]
    InsufficientLiquidity,
    #[msg("Insufficient collateral to borrow")]
    InsufficientCollateral,
    #[msg("Position is healthy, cannot liquidate")]
    PositionHealthy,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Insufficient deposited balance")]
    InsufficientDeposit,
    #[msg("Insufficient borrowed balance")]
    InsufficientBorrow,
    #[msg("Borrow limit exceeded")]
    BorrowLimitExceeded,
    #[msg("Invalid oracle price")]
    InvalidOraclePrice,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Reserve already initialized")]
    ReserveAlreadyInitialized,
    #[msg("Liquidation amount too large")]
    LiquidationAmountTooLarge,
}
