#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
declare_id!("4V9odnGsbuwuZV5WupnzrApKMuRmHSDYwfxH3Rs2CHx6");

#[program]
pub mod staking_program {

    use super::*;

    pub fn initialize(_ctx: Context<Initialize>, reward_rate: u64) -> Result<()> {
        let pool: &mut Account<'_, Pool> = &mut _ctx.accounts.pool;
        pool.owner = _ctx.accounts.signer.key();
        pool.reward_rate = reward_rate;
        pool.total_staked = 0;
        Ok(())
    }

    pub fn stake(_ctx: Context<Stake>, _amount: u64) -> Result<()> {
        let _pool: &mut Account<'_, Pool> = &mut _ctx.accounts.staking_pool;
        let _staker: &mut Account<'_, UserStake> = &mut _ctx.accounts.staker_record;

        token::transfer(
            CpiContext::new(
                _ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: _ctx.accounts.staker_token_wallet.to_account_info(),
                    to: _ctx.accounts.pool_token_vault.to_account_info(),
                    authority: _ctx.accounts.staker.to_account_info(),
                },
            ),
            _amount,
        )?;

        _staker.staked_amount = _staker.staked_amount.checked_add(_amount).unwrap();
        _staker.last_update_timestamp = Clock::get().unwrap().unix_timestamp as u64;
        _pool.total_staked = _pool.total_staked.checked_add(_amount).unwrap();
        Ok(())
    }

    pub fn unstake(_ctx: Context<Unstake>, _amount: u64) -> Result<()> {
        // Calculate rewards using immutable references first
        let current_time = Clock::get()?.unix_timestamp as u64;
        let elapsed_time = current_time
            .checked_sub(_ctx.accounts.staker_record.last_update_timestamp)
            .unwrap();
        let earned_reward = elapsed_time
            .checked_mul(_ctx.accounts.staking_pool.reward_rate)
            .unwrap()
            .checked_mul(_ctx.accounts.staker_record.staked_amount)
            .unwrap()
            / 1_000_000;

        let owner_seeds = &[b"pool".as_ref(), &[_ctx.bumps.staking_pool]];

        // Transfer staked tokens
        token::transfer(
            CpiContext::new_with_signer(
                _ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: _ctx.accounts.pool_token_vault.to_account_info(),
                    to: _ctx.accounts.staker_token_wallet.to_account_info(),
                    authority: _ctx.accounts.staking_pool.to_account_info(),
                },
                &[owner_seeds],
            ),
            _amount,
        )?;

        // Transfer rewards
        if earned_reward > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    _ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: _ctx.accounts.reward_token_vault.to_account_info(),
                        to: _ctx.accounts.staker_token_wallet.to_account_info(),
                        authority: _ctx.accounts.staking_pool.to_account_info(), // Fixed authority
                    },
                    &[owner_seeds],
                ),
                earned_reward,
            )?;
        }

        // Mutate accounts in a scoped block after CPIs
        {
            let _pool = &mut _ctx.accounts.staking_pool;
            let _staker = &mut _ctx.accounts.staker_record;
            _staker.staked_amount = _staker.staked_amount.checked_sub(_amount).unwrap();
            _staker.last_update_timestamp = current_time;
            _pool.total_staked = _pool.total_staked.checked_sub(_amount).unwrap();
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = signer,
        space = 8 + 32 + 8 + 8, // discriminator + owner (Pubkey) + reward_rate (u64) + total_staked (u64)
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut, seeds = [b"pool"], bump)]
    pub staking_pool: Account<'info, Pool>, // Clearly the pool where tokens are staked
    #[account(
        init_if_needed,
        payer = staker,
        space = 8 + 8 + 8, // discriminator + amount (u64) + last_stake_ts (i64)
        seeds = [b"stake", staker.key().as_ref()],
        bump
    )]
    pub staker_record: Account<'info, UserStake>, // Tracks this user's stake
    #[account(mut)]
    pub staker: Signer<'info>, // The person staking, who signs
    #[account(mut)]
    pub staker_token_wallet: Account<'info, TokenAccount>, // Source of tokens
    #[account(mut)]
    pub pool_token_vault: Account<'info, TokenAccount>, // Where tokens are locked
    pub token_program: Program<'info, Token>, // SPL Token program for transfers
    pub system_program: Program<'info, System>, // For account creation
}
#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut, seeds=[b"pool"],bump)]
    pub staking_pool: Account<'info, Pool>,
    #[account(mut, seeds=[b"stake", staker.key().as_ref()],bump)]
    pub staker_record: Account<'info, UserStake>,

    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut)]
    pub staker_token_wallet: Account<'info, TokenAccount>, // Source of tokens
    #[account(mut)]
    pub pool_token_vault: Account<'info, TokenAccount>, // Where tokens are locked
    #[account(mut)]
    pub reward_token_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>, // SPL Token program for transfers
}

#[account]
pub struct Pool {
    pub owner: Pubkey,
    pub reward_rate: u64,
    pub total_staked: u64,
}

#[account]
pub struct UserStake {
    pub staked_amount: u64,
    pub last_update_timestamp: u64,
}
