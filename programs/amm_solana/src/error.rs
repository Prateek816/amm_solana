
use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Fee percentage can only be between 0 and 100 (10,000 basis points).")]
    FeePercentErr,

    #[msg("Default error.")]
    DefaultError,

    #[msg("Offer expired.")]
    OfferExpired,

    #[msg("This pool is locked.")]
    PoolLocked,

    #[msg("Slippage exceeded.")]
    SlippageExceeded,

    #[msg("Overflow detected.")]
    Overflow,

    #[msg("Underflow detected.")]
    Underflow,

    #[msg("Invalid token.")]
    InvalidToken,

    #[msg("Actual liquidity is less than minimum.")]
    LiquidityLessThanMinimum,

    #[msg("No liquidity in pool.")]
    NoLiquidityInPool,

    #[msg("Bump error.")]
    BumpError,

    #[msg("Curve calculation error.")]
    CurveError,

    #[msg("Fee is greater than 100%.")]
    InvalidFee,

    #[msg("Invalid update authority.")]
    InvalidAuthority,

    #[msg("No update authority set.")]
    NoAuthoritySet,

    #[msg("Invalid amount.")]
    InvalidAmount,

    #[msg("Invalid precision.")]
    InvalidPrecision,

    #[msg("Insufficient balance.")]
    InsufficientBalance,

    #[msg("Zero balance.")]
    ZeroBalance,

    #[msg("Arithmetic overflow or underflow.")]
    MathOverflow,
}
