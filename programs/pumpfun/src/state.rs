use crate::constants::LAMPORT_DECIMALS;
use crate::errors::*;
use crate::events::CompleteEvent;
use crate::utils::*;
use anchor_lang::{prelude::*, AnchorDeserialize, AnchorSerialize};
use anchor_spl::token::Mint;
use anchor_spl::token::Token;
use core::fmt::Debug;
use std::ops::Div;
use std::ops::Mul;
use std::ops::Sub;

#[account]
pub struct Config {
    pub authority: Pubkey,
    //  use this for 2 step ownership transfer
    pub pending_authority: Pubkey,

    pub team_wallet: Pubkey,

    pub init_bonding_curve: f64, // bonding curve init percentage. The remaining amount is sent to team wallet for distribution to agent

    pub platform_buy_fee: f64, //  platform fee percentage
    pub platform_sell_fee: f64,
    pub platform_migration_fee: f64,

    pub curve_limit: u64, //  lamports to complete te bonding curve

    pub lamport_amount_config: u64,
    pub token_supply_config: u64,
    pub token_decimals_config: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum AmountConfig<T: PartialEq + PartialOrd + Debug> {
    Range { min: Option<T>, max: Option<T> },
    Enum(Vec<T>),
}

impl<T: PartialEq + PartialOrd + Debug> AmountConfig<T> {
    pub fn validate(&self, value: &T) -> Result<()> {
        match self {
            Self::Range { min, max } => {
                if let Some(min) = min {
                    if value < min {
                        msg!("Value {:?} too small, expected at least {:?}", value, min);
                        return Err(ValueTooSmall.into());
                    }
                }
                if let Some(max) = max {
                    if value > max {
                        msg!("Value {:?} too large, expected at most {:?}", value, max);
                        return Err(ValueTooLarge.into());
                    }
                }

                Ok(())
            }
            Self::Enum(options) => {
                if options.contains(value) {
                    Ok(())
                } else {
                    msg!("Invalid value {:?}, expected one of: {:?}", value, options);
                    Err(ValueInvalid.into())
                }
            }
        }
    }
}

#[account]
pub struct BondingCurve {
    pub token_mint: Pubkey,
    pub creator: Pubkey,

    pub init_lamport: u64,

    pub reserve_lamport: u64,
    pub reserve_token: u64,

    pub is_completed: bool,
}
pub trait BondingCurveAccount<'info> {
    // Updates the token reserves in the liquidity pool
    fn update_reserves(
        &mut self,
        global_config: &Account<'info, Config>,
        reserve_one: u64,
        reserve_two: u64,
    ) -> Result<bool>;

    fn swap(
        &mut self,
        global_config: &Account<'info, Config>,
        token_mint: &Account<'info, Mint>,
        global_ata: &mut AccountInfo<'info>,
        user_ata: &mut AccountInfo<'info>,
        source: &mut AccountInfo<'info>,
        team_wallet: &mut AccountInfo<'info>,
        amount: u64,
        direction: u8,
        minimum_receive_amount: u64,

        user: &Signer<'info>,
        signer: &[&[&[u8]]],

        token_program: &Program<'info, Token>,
        system_program: &Program<'info, System>,
    ) -> Result<u64>;

    fn cal_amount_out(
        &self,
        amount: u64,
        token_one_decimals: u8,
        direction: u8,
        platform_sell_fee: f64,
        platform_buy_fee: f64,
    ) -> Result<(u64, u64)>;
}

impl<'info> BondingCurveAccount<'info> for Account<'info, BondingCurve> {
    fn update_reserves(
        &mut self,
        global_config: &Account<'info, Config>,
        reserve_token: u64,
        reserve_lamport: u64,
    ) -> Result<bool> {
        self.reserve_token = reserve_token;
        self.reserve_lamport = reserve_lamport;

        if reserve_lamport >= global_config.curve_limit {
            msg!("curve is completed");
            self.is_completed = true;
            return Ok(true);
        }

        Ok(false)
    }

    fn swap(
        &mut self,
        global_config: &Account<'info, Config>,

        token_mint: &Account<'info, Mint>,
        global_ata: &mut AccountInfo<'info>,
        user_ata: &mut AccountInfo<'info>,

        source: &mut AccountInfo<'info>,
        team_wallet: &mut AccountInfo<'info>,

        amount: u64,
        direction: u8,
        minimum_receive_amount: u64,

        user: &Signer<'info>,
        signer: &[&[&[u8]]],

        token_program: &Program<'info, Token>,
        system_program: &Program<'info, System>,
    ) -> Result<u64> {
        if amount <= 0 {
            return err!(PumpfunError::InvalidAmount);
        }

        msg!("Mint: {:?} ", token_mint.key());
        msg!("Swap: {:?} {:?} {:?}", user.key(), direction, amount);

        let (fee, amount_out) = self.cal_amount_out(
            amount,
            token_mint.decimals,
            direction,
            global_config.platform_sell_fee,
            global_config.platform_buy_fee,
        )?;

        require!(amount_out >= minimum_receive_amount, PumpfunError::ReturnAmountTooSmall);

        // 1 is selling token
        if direction == 1 {
            let new_reserves_one = self
                .reserve_token
                .checked_add(amount_out)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            let new_reserves_two = self
                .reserve_lamport
                .checked_sub(amount_out + fee)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            self.update_reserves(
                global_config,
                new_reserves_one,
                new_reserves_two
            )?;

            token_transfer_user(
                user_ata.clone(),
                &user,
                global_ata.clone(),
                &token_program,
                amount,
            )?;

            sol_transfer_with_signer(
                source.clone(),
                user.to_account_info(),
                &system_program,
                signer,
                amount_out,
            )?;

            sol_transfer_with_signer(
                source.clone(),
                team_wallet.to_account_info(),
                &system_program,
                signer,
                fee,
            )?;
            
            msg!("fee: {:?} amount_out: {:?}", fee, amount_out);
        } else {
            // buying token with SOL
            let new_reserves_one = self
                .reserve_token
                .checked_sub(amount_out)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            let new_reserves_two = self
                .reserve_lamport
                .checked_add(amount - fee)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            let is_completed = self.update_reserves(
                global_config,
                new_reserves_one,
                new_reserves_two
            )?;

            if is_completed == true {
                emit!(CompleteEvent {
                    user: user.key(),
                    mint: token_mint.key(),
                    bonding_curve: self.key()
                });
            }

            token_transfer_with_signer(
                global_ata.clone(),
                source.clone(),
                user_ata.clone(),
                &token_program,
                signer,
                amount_out,
            )?;

            sol_transfer_from_user(&user, source.clone(), &system_program, amount - fee)?;
            sol_transfer_from_user(&user, team_wallet.clone(), &system_program, fee)?;
            msg!("fee: {:?} amount_out: {:?}", fee, amount_out);
        }
        Ok(amount_out)
    }

    fn cal_amount_out(
        &self,
        amount: u64,
        token_one_decimals: u8,
        direction: u8,
        platform_sell_fee: f64,
        platform_buy_fee: f64,
    ) -> Result<(u64, u64)> {
        // xy = k => Constant product formula
        // formula => dy = ydx / (x + dx)

        let fee_percent = if direction == 1 {
            platform_sell_fee
        } else {
            platform_buy_fee
        };

        let amount_out: u64;
        let fee: u64;

        // sell
        if direction == 1 {
            // sell, token for sol
            // x + dx token
            let denominator_sum = self
                .reserve_token
                .checked_add(amount)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            // (x + dx) / dx
            let div_amt = convert_to_float(denominator_sum, token_one_decimals)
                .div(convert_to_float(amount, token_one_decimals));

            // dy = y / ((x + dx) / dx)
            // dx = ydx / (x + dx)
            let amount_out_total_in_float =
                convert_to_float(self.reserve_lamport, LAMPORT_DECIMALS).div(div_amt);
            let amount_out_total = convert_from_float(amount_out_total_in_float, LAMPORT_DECIMALS);

            let adjusted_amount_out_float = amount_out_total_in_float
                .div(100_f64)
                .mul(100_f64.sub(fee_percent));
            amount_out = convert_from_float(adjusted_amount_out_float, LAMPORT_DECIMALS);
            fee = amount_out_total - amount_out;
        } else {
            // buy, sol for token
            // dy sol
            let adjusted_amount_in_float = convert_to_float(amount, token_one_decimals)
                .div(100_f64)
                .mul(100_f64.sub(fee_percent));
            let adjusted_amount = convert_from_float(adjusted_amount_in_float, token_one_decimals);

            fee = amount - adjusted_amount;

            // y + dy sol
            let denominator_sum = self
                .reserve_lamport
                .checked_add(adjusted_amount)
                .ok_or(PumpfunError::OverflowOrUnderflowOccurred)?;

            // (y + dy) / dy
            let div_amt = convert_to_float(denominator_sum, LAMPORT_DECIMALS)
                .div(convert_to_float(adjusted_amount, LAMPORT_DECIMALS));

            // dx = x / ((y + dy) / dy)
            // dx = xdy / (y + dy)
            let amount_out_in_float =
                convert_to_float(self.reserve_token, token_one_decimals).div(div_amt);
            amount_out = convert_from_float(amount_out_in_float, token_one_decimals);
        }
        Ok((fee, amount_out))
    }
}
