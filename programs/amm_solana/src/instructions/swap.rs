
use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};

use crate::{error::AmmError, state::Config};

const BASIS_POINTS: u128 = 10_000;


#[derive(Accounts)]
pub struct Swap<'info> {

    #[account(mut)]
    pub user: Signer<'info>,

    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,

    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,

    #[account(
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,

    // Pool vault X
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,

    // Pool vault Y
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,

    // User token accounts
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,

    // Treasury X
    #[account(
        mut,
        seeds = [b"treasury_x", config.key().as_ref()],
        bump,
        token::mint = mint_x,
        token::authority = config,
    )]
    pub treasury_x: Box<Account<'info, TokenAccount>>,

    // Treasury Y
    #[account(
        mut,
        seeds = [b"treasury_y", config.key().as_ref()],
        bump,
        token::mint = mint_y,
        token::authority = config,
    )]
    pub treasury_y: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}


impl<'info> Swap<'info> {

    pub fn swap(
        &mut self,
        is_x: bool,
        amount: u64,
        min: u64,
    ) -> Result<()> {

        // 1. Validate input
        require!(amount > 0, AmmError::InvalidAmount);

        require!(!self.config.locked, AmmError::PoolLocked);

        let fee_bps = self.config.fee as u128;
        let protocol_fee_bps = self.config.protocol_fee as u128;

        require!(fee_bps < BASIS_POINTS, AmmError::InvalidFee);

        require!(
            protocol_fee_bps <= fee_bps,
            AmmError::InvalidFee
        );

        // 2. Get reserves based on swap direction
        let (reserve_in, reserve_out) = if is_x {
            (self.vault_x.amount, self.vault_y.amount)
        } else {
            (self.vault_y.amount, self.vault_x.amount)
        };

        require!(
            reserve_in > 0 && reserve_out > 0,
            AmmError::InvalidAmount
        );

        // 3. Calculate trading fee
        let trading_fee = (amount as u128)
            .checked_mul(fee_bps)
            .ok_or(AmmError::MathOverflow)?
            .checked_div(BASIS_POINTS)
            .ok_or(AmmError::MathOverflow)?;

        // 4. Calculate effective input amount
        let effective_amount = (amount as u128)
            .checked_sub(trading_fee)
            .ok_or(AmmError::MathOverflow)?;

        // 5. Calculate output using constant product formula
        //
        // amount_out = (reserve_out * effective_amount)
        //              / (reserve_in + effective_amount)

        let numerator = (reserve_out as u128)
            .checked_mul(effective_amount)
            .ok_or(AmmError::MathOverflow)?;

        let denominator = (reserve_in as u128)
            .checked_add(effective_amount)
            .ok_or(AmmError::MathOverflow)?;

        let amount_out = numerator
            .checked_div(denominator)
            .ok_or(AmmError::MathOverflow)?;

        let amount_out = u64::try_from(amount_out)
            .map_err(|_| AmmError::MathOverflow)?;

        require!(amount_out > 0, AmmError::InvalidAmount);

        // 6. Slippage protection
        require!(
            amount_out >= min,
            AmmError::SlippageExceeded
        );

        // 7. Calculate protocol share of trading fee
        let protocol_fee = if fee_bps == 0 {
            0
        } else {
            trading_fee
                .checked_mul(protocol_fee_bps)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(fee_bps)
                .ok_or(AmmError::MathOverflow)?
        };

        let protocol_fee = u64::try_from(protocol_fee)
            .map_err(|_| AmmError::MathOverflow)?;

        // 8. Amount deposited into pool
        //
        // LPs retain the non-protocol portion of the trading fee.

        let vault_deposit = amount
            .checked_sub(protocol_fee)
            .ok_or(AmmError::MathOverflow)?;

        // 9. User -> Pool
        self.deposit_tokens(is_x, vault_deposit)?;

        // 10. User -> Protocol Treasury
        self.deposit_protocol_fee(is_x, protocol_fee)?;

        // 11. Pool -> User (output token)
        self.withdraw_tokens(is_x, amount_out)?;

        Ok(())
    }


    // Transfer input tokens into pool
    pub fn deposit_tokens(
        &self,
        is_x: bool,
        amount: u64,
    ) -> Result<()> {

        let (from, to) = if is_x {
            (
                self.user_x.to_account_info(),
                self.vault_x.to_account_info(),
            )
        } else {
            (
                self.user_y.to_account_info(),
                self.vault_y.to_account_info(),
            )
        };

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }


    // Transfer protocol fee into treasury
    pub fn deposit_protocol_fee(
        &self,
        is_x: bool,
        amount: u64,
    ) -> Result<()> {

        if amount == 0 {
            return Ok(());
        }

        let (from, to) = if is_x {
            (
                self.user_x.to_account_info(),
                self.treasury_x.to_account_info(),
            )
        } else {
            (
                self.user_y.to_account_info(),
                self.treasury_y.to_account_info(),
            )
        };

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }


    // Transfer output tokens from pool to user
    pub fn withdraw_tokens(
        &self,
        is_x: bool,
        amount: u64,
    ) -> Result<()> {

        let (from, to) = if is_x {
            (
                self.vault_y.to_account_info(),
                self.user_y.to_account_info(),
            )
        } else {
            (
                self.vault_x.to_account_info(),
                self.user_x.to_account_info(),
            )
        };

        let seed_bytes = self.config.seed.to_le_bytes();
        let bump = [self.config.config_bump];

        let signer_seeds: &[&[&[u8]]] = &[&[
            b"config",
            seed_bytes.as_ref(),
            bump.as_ref(),
        ]];

        transfer(
            CpiContext::new_with_signer(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )
    }
}
