use std::io::Cursor;

use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Transfer};

use crate::contexts::Finalize;
use crate::errors::PactError;
use crate::events::{ChallengeSettled, CommitmentScoreUpdated};
use crate::logic::{
    calculate_score_delta, calculate_settlement, clamp_commitment_score, ScoreOutcome,
};
use crate::state::{ChallengeStatus, CommitmentProfile, ParticipantState, SubmissionResult};

pub fn finalize<'info>(ctx: Context<'_, '_, 'info, 'info, Finalize<'info>>) -> Result<()> {
    let challenge = &mut ctx.accounts.challenge;
    let clock = Clock::get()?;

    require!(challenge.status == ChallengeStatus::Active, PactError::CannotFinalize);
    require!(
        clock.unix_timestamp > challenge.submission_deadline,
        PactError::SubmissionWindowNotClosed
    );

    let participant_count = challenge.participant_count as usize;
    let remaining = &ctx.remaining_accounts;
    require!(
        remaining.len() == participant_count * 3,
        PactError::InvalidAccountCount
    );

    let total_pool = challenge.stake_amount * participant_count as u64;
    let other_count = if participant_count > 1 {
        participant_count as u8 - 1
    } else {
        1
    };
    let dispute_threshold = other_count / 2;

    let mut winner_indices: Vec<usize> = Vec::new();
    let mut outcomes: Vec<ScoreOutcome> = Vec::with_capacity(participant_count);

    for i in 0..participant_count {
        let state_info = &remaining[i * 3];
        let state_data = state_info.try_borrow_data()?;
        let state = ParticipantState::try_from_slice(&state_data[8..])?;
        let token_info = &remaining[i * 3 + 1];
        let token_account = Account::<TokenAccount>::try_from(token_info)?;
        let profile_info = &remaining[i * 3 + 2];
        let profile_data = profile_info.try_borrow_data()?;
        let profile = CommitmentProfile::try_from_slice(&profile_data[8..])?;

        require!(state.challenge == challenge.key(), PactError::InvalidParticipant);
        require!(
            token_account.owner == state.participant,
            PactError::InvalidPayoutAccount
        );
        require!(
            token_account.mint == ctx.accounts.vault.mint,
            PactError::InvalidPayoutMint
        );
        require!(
            profile.owner == state.participant,
            PactError::InvalidCommitmentProfile
        );

        let outcome = if !state.submitted {
            ScoreOutcome::AutoFail
        } else if state.submission_result == SubmissionResult::Success {
            if state.dispute_count <= dispute_threshold {
                ScoreOutcome::Win
            } else {
                ScoreOutcome::DisputedFail
            }
        } else {
            ScoreOutcome::Fail
        };

        if outcome == ScoreOutcome::Win {
            winner_indices.push(i);
        }
        outcomes.push(outcome);
    }

    let challenge_id = challenge.challenge_id.clone();
    let bump = challenge.bump;
    let bump_bytes = [bump];
    let seeds: &[&[u8]] = &[b"challenge", challenge_id.as_bytes(), &bump_bytes];
    let signer = &[seeds];

    let winner_count = winner_indices.len();
    let settlement = calculate_settlement(challenge.stake_amount, participant_count, winner_count);

    if winner_indices.is_empty() {
        if settlement.fee > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: ctx.accounts.treasury_token_account.to_account_info(),
                        authority: challenge.to_account_info(),
                    },
                    signer,
                ),
                settlement.fee,
            )?;
        }

        for i in 0..participant_count {
            let token_acct = &remaining[i * 3 + 1];
            let payout = settlement.reward_per_winner
                + if i < settlement.reward_remainder as usize {
                    1
                } else {
                    0
                };
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: token_acct.to_account_info(),
                        authority: challenge.to_account_info(),
                    },
                    signer,
                ),
                payout,
            )?;
        }
    } else {
        if settlement.fee > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: ctx.accounts.treasury_token_account.to_account_info(),
                        authority: challenge.to_account_info(),
                    },
                    signer,
                ),
                settlement.fee,
            )?;
        }

        for (winner_position, &idx) in winner_indices.iter().enumerate() {
            let token_acct = &remaining[idx * 3 + 1];
            let payout = settlement.reward_per_winner
                + if winner_position < settlement.reward_remainder as usize {
                    1
                } else {
                    0
                };
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: token_acct.to_account_info(),
                        authority: challenge.to_account_info(),
                    },
                    signer,
                ),
                payout,
            )?;
        }
    }

    for i in 0..participant_count {
        let state_info = &remaining[i * 3];
        let boost_enabled = {
            let mut state_data = state_info.try_borrow_mut_data()?;
            let mut state = ParticipantState::try_from_slice(&state_data[8..])?;
            state.is_winner = winner_indices.contains(&i);
            let boost_enabled = state.boost_enabled;
            let mut cursor = Cursor::new(&mut state_data[8..]);
            state.try_serialize(&mut cursor)?;
            boost_enabled
        };

        let profile_info = &remaining[i * 3 + 2];
        let (owner, delta, new_score) = {
            let mut profile_data = profile_info.try_borrow_mut_data()?;
            let mut profile = CommitmentProfile::try_from_slice(&profile_data[8..])?;
            let delta = calculate_score_delta(boost_enabled, outcomes[i]);
            profile.commitment_score =
                clamp_commitment_score(profile.commitment_score.saturating_add(delta));
            profile.active_challenge_count = profile.active_challenge_count.saturating_sub(1);
            let owner = profile.owner;
            let new_score = profile.commitment_score;
            let mut profile_cursor = Cursor::new(&mut profile_data[8..]);
            profile.try_serialize(&mut profile_cursor)?;
            (owner, delta, new_score)
        };

        emit!(CommitmentScoreUpdated {
            owner,
            challenge_key: challenge.key(),
            delta,
            new_score,
            boost_enabled,
        });
    }

    challenge.status = ChallengeStatus::Settled;
    emit!(ChallengeSettled {
        challenge_key: challenge.key(),
        winner_count: winner_count as u8,
        total_pool,
        fee: settlement.fee,
        penalty_pool: settlement.penalty_pool,
        reward_per_winner: settlement.reward_per_winner,
        reward_remainder: settlement.reward_remainder,
        all_win: settlement.all_win,
        all_fail: settlement.all_fail,
    });

    Ok(())
}
