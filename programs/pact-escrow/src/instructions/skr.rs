use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::constants::*;
use crate::contexts::{LockSkr, UnlockSkr};
use crate::errors::PactError;
use crate::events::{SkrLocked, SkrUnlocked};

pub fn lock_skr(ctx: Context<LockSkr>, amount: u64) -> Result<()> {
    require!(amount > 0, PactError::InvalidSkrLockAmount);

    let lock_account = &mut ctx.accounts.skr_lock;
    let commitment_profile = &mut ctx.accounts.commitment_profile;
    let clock = Clock::get()?;

    let new_total = lock_account.amount_locked.saturating_add(amount);
    require!(new_total >= MIN_SKR_LOCK, PactError::InsufficientSkrLock);

    if lock_account.owner == Pubkey::default() {
        lock_account.owner = ctx.accounts.user.key();
        lock_account.skr_mint = ctx.accounts.skr_mint.key();
        lock_account.bump = ctx.bumps.skr_lock;
        lock_account.vault_bump = ctx.bumps.skr_vault;
    } else {
        require!(
            lock_account.owner == ctx.accounts.user.key(),
            PactError::InvalidSkrLockOwner
        );
        require!(
            lock_account.skr_mint == ctx.accounts.skr_mint.key(),
            PactError::InvalidSkrMint
        );
    }

    if commitment_profile.owner == Pubkey::default() {
        commitment_profile.owner = ctx.accounts.user.key();
        commitment_profile.commitment_score = DEFAULT_COMMITMENT_SCORE;
        commitment_profile.bump = ctx.bumps.commitment_profile;
    } else {
        require!(
            commitment_profile.owner == ctx.accounts.user.key(),
            PactError::InvalidCommitmentProfile
        );
    }

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_skr_token_account.to_account_info(),
                to: ctx.accounts.skr_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
    )?;

    lock_account.amount_locked = new_total;
    lock_account.lock_timestamp = clock.unix_timestamp;

    emit!(SkrLocked {
        owner: ctx.accounts.user.key(),
        amount_locked: lock_account.amount_locked,
    });

    Ok(())
}

pub fn unlock_skr(ctx: Context<UnlockSkr>, amount: u64) -> Result<()> {
    require!(amount > 0, PactError::InvalidSkrUnlockAmount);

    let lock_account = &mut ctx.accounts.skr_lock;
    let commitment_profile = &ctx.accounts.commitment_profile;

    require!(
        commitment_profile.owner == ctx.accounts.user.key(),
        PactError::InvalidCommitmentProfile
    );
    require!(
        commitment_profile.active_challenge_count == 0,
        PactError::ActiveChallengeParticipation
    );
    require!(amount <= lock_account.amount_locked, PactError::InsufficientSkrBalance);

    let remaining = lock_account.amount_locked - amount;
    require!(
        remaining == 0 || remaining >= MIN_SKR_LOCK,
        PactError::InsufficientSkrLock
    );

    let user_key = ctx.accounts.user.key();
    let bump = lock_account.bump;
    let bump_bytes = [bump];
    let seeds: &[&[u8]] = &[b"skr_lock", user_key.as_ref(), &bump_bytes];
    let signer = &[seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.skr_vault.to_account_info(),
                to: ctx.accounts.user_skr_token_account.to_account_info(),
                authority: lock_account.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    lock_account.amount_locked = remaining;

    emit!(SkrUnlocked {
        owner: user_key,
        amount_locked: lock_account.amount_locked,
    });

    Ok(())
}
