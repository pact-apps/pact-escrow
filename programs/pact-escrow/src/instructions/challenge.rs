use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::constants::*;
use crate::contexts::{
    CreateChallenge, Dispute, JoinChallenge, StartChallenge, SubmitResult, VoteContinue,
};
use crate::errors::PactError;
use crate::events::{ChallengeCreated, DisputeFiled, ParticipantJoined, ResultSubmitted};
use crate::state::{ChallengeStatus, ParticipantState, SkrLockAccount, SubmissionResult};

pub fn create_challenge(
    ctx: Context<CreateChallenge>,
    challenge_id: String,
    stake_amount: u64,
    duration_seconds: i64,
    max_participants: u8,
    title: String,
) -> Result<()> {
    require!(stake_amount > 0, PactError::InvalidStake);
    require!(max_participants >= 2, PactError::TooFewParticipants);
    require!(max_participants as usize <= MAX_PARTICIPANTS, PactError::TooManyParticipants);
    require!(duration_seconds > 0, PactError::InvalidDuration);
    require!(title.len() <= 64, PactError::TitleTooLong);

    let challenge = &mut ctx.accounts.challenge;
    let clock = Clock::get()?;

    challenge.creator = ctx.accounts.creator.key();
    challenge.challenge_id = challenge_id;
    challenge.title = title;
    challenge.stake_amount = stake_amount;
    challenge.max_participants = max_participants;
    challenge.duration_seconds = duration_seconds;
    challenge.created_at = clock.unix_timestamp;
    challenge.deadline = 0;
    challenge.submission_deadline = 0;
    challenge.status = ChallengeStatus::Open;
    challenge.cycle = 1;
    challenge.participant_count = 0;
    challenge.deposit_count = 0;
    challenge.bump = ctx.bumps.challenge;
    challenge.vault_bump = ctx.bumps.vault;

    emit!(ChallengeCreated {
        challenge_key: challenge.key(),
        creator: challenge.creator,
        stake_amount,
        title: challenge.title.clone(),
    });

    Ok(())
}

pub fn join_challenge(ctx: Context<JoinChallenge>) -> Result<()> {
    let challenge = &mut ctx.accounts.challenge;
    let participant_state = &mut ctx.accounts.participant_state;
    let commitment_profile = &mut ctx.accounts.commitment_profile;
    let clock = Clock::get()?;

    require!(challenge.status == ChallengeStatus::Open, PactError::ChallengeNotOpen);
    require!(
        challenge.participant_count < challenge.max_participants,
        PactError::ChallengeFull
    );

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.participant_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.participant.to_account_info(),
            },
        ),
        challenge.stake_amount,
    )?;

    let expected_skr_lock = Pubkey::find_program_address(
        &[b"skr_lock", ctx.accounts.participant.key().as_ref()],
        &crate::id(),
    )
    .0;
    let boost_enabled = if !ctx.accounts.skr_lock.data_is_empty() {
        require!(
            ctx.accounts.skr_lock.key() == expected_skr_lock,
            PactError::InvalidSkrLockAccount
        );
        require!(
            ctx.accounts.skr_lock.owner == &crate::id(),
            PactError::InvalidSkrLockAccount
        );
        let skr_lock_account = ctx.accounts.skr_lock.to_account_info();
        let skr_lock_data_ref = skr_lock_account.try_borrow_data()?;
        let mut skr_lock_data: &[u8] = &skr_lock_data_ref;
        let skr_lock = SkrLockAccount::try_deserialize(&mut skr_lock_data)?;
        require!(
            skr_lock.owner == ctx.accounts.participant.key(),
            PactError::InvalidSkrLockOwner
        );
        skr_lock.amount_locked >= BOOST_THRESHOLD
    } else {
        false
    };

    participant_state.participant = ctx.accounts.participant.key();
    participant_state.challenge = challenge.key();
    participant_state.deposited = true;
    participant_state.submitted = false;
    participant_state.submission_result = SubmissionResult::None;
    participant_state.disputed = false;
    participant_state.dispute_count = 0;
    participant_state.is_winner = false;
    participant_state.boost_enabled = boost_enabled;
    participant_state.bump = ctx.bumps.participant_state;

    if commitment_profile.owner == Pubkey::default() {
        commitment_profile.owner = ctx.accounts.participant.key();
        commitment_profile.commitment_score = DEFAULT_COMMITMENT_SCORE;
        commitment_profile.bump = ctx.bumps.commitment_profile;
    } else {
        require!(
            commitment_profile.owner == ctx.accounts.participant.key(),
            PactError::InvalidCommitmentProfile
        );
    }
    commitment_profile.active_challenge_count =
        commitment_profile.active_challenge_count.saturating_add(1);

    challenge.participant_count += 1;
    challenge.deposit_count += 1;

    if challenge.participant_count == challenge.max_participants {
        challenge.status = ChallengeStatus::Active;
        challenge.deadline = clock.unix_timestamp + challenge.duration_seconds;
        challenge.submission_deadline = challenge.deadline + SUBMISSION_WINDOW_SECONDS;
    }

    emit!(ParticipantJoined {
        challenge_key: challenge.key(),
        participant: ctx.accounts.participant.key(),
        total_participants: challenge.participant_count,
        boost_enabled,
    });

    Ok(())
}

pub fn start_challenge(ctx: Context<StartChallenge>) -> Result<()> {
    let challenge = &mut ctx.accounts.challenge;
    let clock = Clock::get()?;

    require!(challenge.status == ChallengeStatus::Open, PactError::ChallengeNotOpen);
    require!(
        ctx.accounts.creator.key() == challenge.creator,
        PactError::NotCreator
    );
    require!(challenge.participant_count >= 2, PactError::TooFewParticipants);

    challenge.status = ChallengeStatus::Active;
    challenge.deadline = clock.unix_timestamp + challenge.duration_seconds;
    challenge.submission_deadline = challenge.deadline + SUBMISSION_WINDOW_SECONDS;

    Ok(())
}

pub fn submit_result(
    ctx: Context<SubmitResult>,
    success: bool,
    proof_hash: [u8; 32],
) -> Result<()> {
    let challenge = &ctx.accounts.challenge;
    let participant_state = &mut ctx.accounts.participant_state;
    let clock = Clock::get()?;

    require!(
        challenge.status == ChallengeStatus::Active
            || challenge.status == ChallengeStatus::Submission,
        PactError::NotInSubmissionWindow
    );
    require!(
        clock.unix_timestamp >= challenge.deadline,
        PactError::ChallengeNotEnded
    );
    require!(
        clock.unix_timestamp <= challenge.submission_deadline,
        PactError::SubmissionWindowClosed
    );
    require!(participant_state.deposited, PactError::NotDeposited);
    require!(!participant_state.submitted, PactError::AlreadySubmitted);

    participant_state.submitted = true;
    participant_state.submission_result = if success {
        SubmissionResult::Success
    } else {
        SubmissionResult::Fail
    };
    participant_state.proof_hash = proof_hash;

    emit!(ResultSubmitted {
        challenge_key: challenge.key(),
        participant: ctx.accounts.participant.key(),
        success,
    });

    Ok(())
}

pub fn dispute(ctx: Context<Dispute>) -> Result<()> {
    let challenge = &ctx.accounts.challenge;
    let target_state = &mut ctx.accounts.target_participant_state;
    let clock = Clock::get()?;

    require!(
        clock.unix_timestamp >= challenge.deadline,
        PactError::ChallengeNotEnded
    );
    require!(
        clock.unix_timestamp <= challenge.submission_deadline,
        PactError::SubmissionWindowClosed
    );
    require!(
        ctx.accounts.disputer.key() != target_state.participant,
        PactError::CannotDisputeSelf
    );

    let dispute_receipt = &mut ctx.accounts.dispute_receipt;
    dispute_receipt.challenge = challenge.key();
    dispute_receipt.disputer = ctx.accounts.disputer.key();
    dispute_receipt.target = target_state.participant;
    dispute_receipt.bump = ctx.bumps.dispute_receipt;

    target_state.dispute_count += 1;

    emit!(DisputeFiled {
        challenge_key: challenge.key(),
        disputer: ctx.accounts.disputer.key(),
        target: target_state.participant,
        total_disputes: target_state.dispute_count,
    });

    Ok(())
}

pub fn vote_continue(ctx: Context<VoteContinue>, wants_continue: bool) -> Result<()> {
    let challenge = &ctx.accounts.challenge;
    let participant_state: &mut Account<'_, ParticipantState> = &mut ctx.accounts.participant_state;

    require!(
        challenge.status == ChallengeStatus::AllFailVoting,
        PactError::NotInVotingPhase
    );
    require!(participant_state.deposited, PactError::NotDeposited);

    participant_state.vote_continue = Some(wants_continue);

    Ok(())
}
