use anchor_lang::{prelude::*, solana_program::program::invoke_signed};
use anchor_spl::token::{burn, Burn, TokenAccount};

use crate::{
    amm_instruction, constants::{BONDING_CURVE, CONFIG, GLOBAL}, errors::PumpfunError, events::MigrateEvent, state::{BondingCurve, BondingCurveAccount, Config}
};

#[derive(Accounts)]
pub struct Migrate<'info> {
    /// CHECK: Safe
    #[account(
        mut,
        constraint = global_config.team_wallet == *team_wallet.key @PumpfunError::IncorrectAuthority
    )]
    team_wallet: UncheckedAccount<'info>,

    #[account(
        seeds = [CONFIG.as_bytes()],
        bump,
    )]
    global_config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [BONDING_CURVE.as_bytes(), &coin_mint.key().to_bytes()],
        bump
    )]
    bonding_curve: Box<Account<'info, BondingCurve>>,

    /// CHECK
    #[account(
        mut,
        seeds = [GLOBAL.as_bytes()],
        bump,
    )]
    global_vault: UncheckedAccount<'info>,

    /// CHECK: Safe
    amm_program: UncheckedAccount<'info>,

    /// CHECK: Safe. The spl token program
    // token_program: Program<'info, Token>,
    token_program: UncheckedAccount<'info>,

    /// CHECK: Safe. The associated token program
    associated_token_program: UncheckedAccount<'info>,
    // associated_token_program: Program<'info, AssociatedToken>,
    /// CHECK: Safe. System program
    // system_program: Program<'info, System>,
    system_program: UncheckedAccount<'info>,

    /// CHECK: Safe. Rent program
    // sysvar_rent: Sysvar<'info, Rent>,
    sysvar_rent: UncheckedAccount<'info>,

    /// CHECK: Safe.
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"amm_associated_seed"],
        bump,
        seeds::program = amm_program.key()
    )]
    amm: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        seeds = [b"amm authority"],
        bump,
        seeds::program = amm_program.key()
    )]
    amm_authority: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"open_order_associated_seed"],
        bump,
        seeds::program = amm_program.key()
    )]
    amm_open_orders: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"lp_mint_associated_seed"
        ],
        bump,
        seeds::program = amm_program.key()
    )]
    lp_mint: UncheckedAccount<'info>,

    #[account(mut)]
    coin_mint: UncheckedAccount<'info>,

    /// CHECK: Safe. Pc mint account
    pc_mint: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"coin_vault_associated_seed"
        ],
        bump,
        seeds::program = amm_program.key()
    )]
    coin_vault: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"pc_vault_associated_seed"
        ],
        bump,
        seeds::program = amm_program.key()
    )]
    pc_vault: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [
            amm_program.key.as_ref(),
            market.key.as_ref(),
            b"target_associated_seed"
        ],
        bump,
        seeds::program = amm_program.key()
    )]
    target_orders: UncheckedAccount<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        seeds = [b"amm_config_account_seed"],
        bump,
        seeds::program = amm_program.key()
    )]
    amm_config: UncheckedAccount<'info>,

    /// CHECK: Safe. OpenBook program.
    market_program: UncheckedAccount<'info>,
    /// CHECK: Safe. OpenBook market. OpenBook program is the owner.
    #[account(mut)]
    market: UncheckedAccount<'info>,

    /// CHECK: Safe. OpenBook market. OpenBook program is the owner.
    #[account(mut)]
    fee_destination: UncheckedAccount<'info>,

    /// CHECK: Safe. The user wallet create the pool
    #[account(mut)]
    payer: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = coin_mint,
        associated_token::authority = global_vault
    )]
    global_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = coin_mint,
        associated_token::authority = team_wallet
    )]
    team_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = pc_mint,
        associated_token::authority = global_vault
    )]
    global_wsol_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Safe. lp token account of global_vault
    #[account(
        mut,
        seeds = [
            global_vault.key().as_ref(),
            anchor_spl::token::spl_token::ID.as_ref(),
            lp_mint.key().as_ref(),
        ],
        bump,
        seeds::program = anchor_spl::associated_token::ID
    )]
    global_lp_account: UncheckedAccount<'info>,
}

impl<'info> Migrate<'info> {
    pub fn process(&mut self, nonce: u8, global_vault_bump: u8) -> Result<()> {
        let bonding_curve = &mut self.bonding_curve;

        //  check curve is completed
        require!(
            bonding_curve.is_completed == true,
            PumpfunError::CurveNotCompleted
        );

        let init_pc_amount = self.global_wsol_account.amount;

        let coin_amount = self.global_token_account.amount;

        msg!(
            "Raydium Input:: Token: {:?}  Sol: {:?}",
            coin_amount,
            init_pc_amount
        );

        let signer_seeds: &[&[&[u8]]] = &[&[GLOBAL.as_bytes(), &[global_vault_bump]]];

        //  Running raydium amm initialize2
        let initialize_ix = amm_instruction::initialize2(
            self.amm_program.key,
            self.amm.key,
            self.amm_authority.key,
            self.amm_open_orders.key,
            self.lp_mint.key,
            &self.coin_mint.key(),
            &self.pc_mint.key(),
            self.coin_vault.key,
            self.pc_vault.key,
            self.target_orders.key,
            self.amm_config.key,
            self.fee_destination.key,
            self.market_program.key,
            self.market.key,
            self.global_vault.key,
            self.global_token_account.to_account_info().key,
            &self.global_wsol_account.key(),
            &self.global_lp_account.key(),
            nonce,
            Clock::get()?.unix_timestamp as u64,
            init_pc_amount,
            coin_amount,
        )?;

        let account_infos = [
            self.amm_program.to_account_info(),
            self.amm.to_account_info(),
            self.amm_authority.to_account_info(),
            self.amm_open_orders.to_account_info(),
            self.lp_mint.to_account_info(),
            self.coin_mint.to_account_info(),
            self.pc_mint.to_account_info(),
            self.coin_vault.to_account_info(),
            self.pc_vault.to_account_info(),
            self.target_orders.to_account_info(),
            self.amm_config.to_account_info(),
            self.fee_destination.to_account_info(),
            self.market_program.to_account_info(),
            self.market.to_account_info(),
            self.global_vault.to_account_info(),
            self.global_token_account.to_account_info(),
            self.global_wsol_account.to_account_info(),
            self.global_lp_account.to_account_info(),
            self.token_program.to_account_info(),
            self.system_program.to_account_info(),
            self.associated_token_program.to_account_info(),
            self.sysvar_rent.to_account_info(),
        ];
        invoke_signed(&initialize_ix, &account_infos, signer_seeds)?;

        //  Burn LP token
        let burn_ctx = CpiContext::new(
            self.token_program.to_account_info(),
            Burn {
                mint: self.lp_mint.to_account_info().clone(),
                from: self.global_lp_account.to_account_info().clone(),
                authority: self.global_vault.to_account_info().clone(),
            },
        );

        let lp_acc =
            TokenAccount::try_deserialize(&mut &**self.global_lp_account.try_borrow_mut_data()?)?;
        burn(burn_ctx.with_signer(signer_seeds), lp_acc.amount)?;

        //  update reserves
        bonding_curve.update_reserves(&*self.global_config, 0, 0)?;

        //  emit an event
        emit!(MigrateEvent {
            admin: self.payer.key(),
            token: self.coin_mint.key(),
            bonding_curve: self.bonding_curve.key(),
            token_in: coin_amount,
            sol_in: init_pc_amount
        });

        Ok(())
    }
}
