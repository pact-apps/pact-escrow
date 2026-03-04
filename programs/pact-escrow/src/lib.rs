use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("6JTfaG74DZydUwHdQAo6P6frYAVhSitwTexomvTphbCs");

pub const MAX_PARTICIPANTS: usize = 10;
pub const SETTLEMENT_FEE_BPS: u64 = 200; // 2%
pub const MAX_CYCLES: u8 = 2;
pub const SUBMISSION_WINDOW_SECONDS: i64 = 86_400; // 24 hours
pub const DISPUTE_THRESHOLD_BPS: u64 = 5_000; // 50% — disputes above this → fail

// ============================================================
// PROGRAM
// ============================================================
#[program]
pub mod pact_escrow {
    use super::*;

    /// Buat challenge baru
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
        challenge.deadline = 0; // set when challenge starts
        challenge.submission_deadline = 0; // set when challenge starts
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

    /// Join challenge + deposit USDC
    pub fn join_challenge(ctx: Context<JoinChallenge>) -> Result<()> {
        let challenge = &mut ctx.accounts.challenge;
        let participant_state = &mut ctx.accounts.participant_state;
        let clock = Clock::get()?;

        require!(
            challenge.status == ChallengeStatus::Open,
            PactError::ChallengeNotOpen
        );
        require!(
            challenge.participant_count < challenge.max_participants,
            PactError::ChallengeFull
        );

        // Transfer USDC ke vault
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.participant_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.participant.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, challenge.stake_amount)?;

        // Setup participant state
        participant_state.participant = ctx.accounts.participant.key();
        participant_state.challenge = challenge.key();
        participant_state.deposited = true;
        participant_state.submitted = false;
        participant_state.submission_result = SubmissionResult::None;
        participant_state.disputed = false;
        participant_state.dispute_count = 0;
        participant_state.is_winner = false;
        participant_state.bump = ctx.bumps.participant_state;

        challenge.participant_count += 1;
        challenge.deposit_count += 1;

        // Auto-start jika semua slot terisi (atau bisa juga manual start)
        if challenge.participant_count == challenge.max_participants {
            challenge.status = ChallengeStatus::Active;
            challenge.deadline = clock.unix_timestamp + challenge.duration_seconds;
            challenge.submission_deadline =
                challenge.deadline + SUBMISSION_WINDOW_SECONDS;
        }

        emit!(ParticipantJoined {
            challenge_key: challenge.key(),
            participant: ctx.accounts.participant.key(),
            total_participants: challenge.participant_count,
        });

        Ok(())
    }

    /// Start challenge manually (oleh creator, min 2 participants)
    pub fn start_challenge(ctx: Context<StartChallenge>) -> Result<()> {
        let challenge = &mut ctx.accounts.challenge;
        let clock = Clock::get()?;

        require!(
            challenge.status == ChallengeStatus::Open,
            PactError::ChallengeNotOpen
        );
        require!(
            ctx.accounts.creator.key() == challenge.creator,
            PactError::NotCreator
        );
        require!(
            challenge.participant_count >= 2,
            PactError::TooFewParticipants
        );

        challenge.status = ChallengeStatus::Active;
        challenge.deadline = clock.unix_timestamp + challenge.duration_seconds;
        challenge.submission_deadline = challenge.deadline + SUBMISSION_WINDOW_SECONDS;

        Ok(())
    }

    /// Submit hasil (Success atau Fail) + proof hash
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

        // Cek window: setelah deadline, sebelum submission_deadline
        require!(
            clock.unix_timestamp >= challenge.deadline,
            PactError::ChallengeNotEnded
        );
        require!(
            clock.unix_timestamp <= challenge.submission_deadline,
            PactError::SubmissionWindowClosed
        );
        require!(
            participant_state.deposited,
            PactError::NotDeposited
        );
        require!(
            !participant_state.submitted,
            PactError::AlreadySubmitted
        );

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

    /// Dispute hasil peserta lain
    pub fn dispute(ctx: Context<Dispute>) -> Result<()> {
        let challenge = &ctx.accounts.challenge;
        let target_state = &mut ctx.accounts.target_participant_state;
        let clock = Clock::get()?;

        // Dispute hanya bisa saat submission window
        require!(
            clock.unix_timestamp >= challenge.deadline,
            PactError::ChallengeNotEnded
        );
        require!(
            clock.unix_timestamp <= challenge.submission_deadline,
            PactError::SubmissionWindowClosed
        );

        // Tidak bisa dispute diri sendiri
        require!(
            ctx.accounts.disputer.key() != target_state.participant,
            PactError::CannotDisputeSelf
        );

        target_state.dispute_count += 1;

        emit!(DisputeFiled {
            challenge_key: challenge.key(),
            disputer: ctx.accounts.disputer.key(),
            target: target_state.participant,
            total_disputes: target_state.dispute_count,
        });

        Ok(())
    }

    /// Finalize / Settlement — permissionless, siapa saja bisa trigger
    ///
    /// Logic:
    /// 1. Non-submitters → auto-fail
    /// 2. Submitted Success but disputed by >50% of others → fail
    /// 3. Rest → winner
    /// 4. Distribution:
    ///    - Has winners → 2% fee, rest split to winners
    ///    - All fail cycle 1 → enter AllFailVoting phase
    ///    - All fail cycle 2+ → refund with 2% fee
    ///
    /// remaining_accounts layout:
    ///   [ParticipantState_0, TokenAccount_0, ParticipantState_1, TokenAccount_1, ...]
    pub fn finalize<'info>(ctx: Context<'_, '_, 'info, 'info, Finalize<'info>>) -> Result<()> {
        let challenge = &mut ctx.accounts.challenge;
        let clock = Clock::get()?;

        require!(
            challenge.status == ChallengeStatus::Active
                || challenge.status == ChallengeStatus::Submission,
            PactError::CannotFinalize
        );
        require!(
            clock.unix_timestamp > challenge.submission_deadline,
            PactError::SubmissionWindowNotClosed
        );

        let participant_count = challenge.participant_count as usize;
        let remaining = &ctx.remaining_accounts;

        // Each participant needs: ParticipantState + TokenAccount
        require!(
            remaining.len() == participant_count * 2,
            PactError::InvalidAccountCount
        );

        let total_pool = challenge.stake_amount * participant_count as u64;
        let other_count = if participant_count > 1 {
            participant_count as u8 - 1
        } else {
            1
        };
        // >50% of other participants must dispute for it to count
        let dispute_threshold = other_count / 2;

        // First pass: determine winners by deserializing each ParticipantState
        let mut winner_indices: Vec<usize> = Vec::new();

        for i in 0..participant_count {
            let state_info = &remaining[i * 2];
            let state_data = state_info.try_borrow_data()?;
            // Skip 8-byte discriminator
            let state = ParticipantState::try_from_slice(&state_data[8..])?;

            // Must belong to this challenge
            require!(
                state.challenge == challenge.key(),
                PactError::InvalidParticipant
            );

            let is_winner = if !state.submitted {
                false // auto-fail: never submitted
            } else if state.submission_result == SubmissionResult::Success {
                // Not disputed by majority
                state.dispute_count <= dispute_threshold
            } else {
                false // self-reported failure
            };

            if is_winner {
                winner_indices.push(i);
            }
        }

        // PDA signer seeds for vault transfers (challenge is vault authority)
        let challenge_id = challenge.challenge_id.clone();
        let bump = challenge.bump;
        let bump_bytes = [bump];
        let seeds: &[&[u8]] = &[b"challenge", challenge_id.as_bytes(), &bump_bytes];
        let signer = &[seeds];

        if winner_indices.is_empty() {
            if challenge.cycle < MAX_CYCLES {
                // All fail cycle 1: enter voting phase
                challenge.status = ChallengeStatus::AllFailVoting;
                challenge.cycle += 1;
                return Ok(());
            }
            // All fail final cycle: refund with fee
            let fee = total_pool * SETTLEMENT_FEE_BPS / 10_000;
            let distributable = total_pool - fee;

            // Transfer fee to treasury
            if fee > 0 {
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
                    fee,
                )?;
            }

            // Refund each participant equally
            let per_person = distributable / participant_count as u64;
            for i in 0..participant_count {
                let token_acct = &remaining[i * 2 + 1];
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
                    per_person,
                )?;
            }
        } else {
            // Has winners: 2% fee, rest to winners
            let fee = total_pool * SETTLEMENT_FEE_BPS / 10_000;
            let distributable = total_pool - fee;

            // Transfer fee to treasury
            if fee > 0 {
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
                    fee,
                )?;
            }

            // Split to winners
            let per_winner = distributable / winner_indices.len() as u64;
            for &idx in &winner_indices {
                let token_acct = &remaining[idx * 2 + 1];
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
                    per_winner,
                )?;
            }
        }

        // Second pass: update is_winner flags on participant states
        for i in 0..participant_count {
            let state_info = &remaining[i * 2];
            let mut state_data = state_info.try_borrow_mut_data()?;
            // is_winner is at a known offset in the struct:
            // discriminator(8) + participant(32) + challenge(32) + deposited(1) +
            // submitted(1) + submission_result(1) + proof_hash(32) + disputed(1) +
            // dispute_count(1) = offset 109
            let is_winner_offset = 8 + 32 + 32 + 1 + 1 + 1 + 32 + 1 + 1;
            state_data[is_winner_offset] = if winner_indices.contains(&i) { 1 } else { 0 };
        }

        challenge.status = ChallengeStatus::Settled;

        let fee_val = total_pool * SETTLEMENT_FEE_BPS / 10_000;
        emit!(ChallengeSettled {
            challenge_key: challenge.key(),
            winner_count: winner_indices.len() as u8,
            total_pool,
            fee: fee_val,
        });

        Ok(())
    }

    /// All-fail cycle 1: vote untuk continue atau refund
    pub fn vote_continue(ctx: Context<VoteContinue>, wants_continue: bool) -> Result<()> {
        let challenge = &ctx.accounts.challenge;
        let participant_state = &mut ctx.accounts.participant_state;

        require!(
            challenge.status == ChallengeStatus::AllFailVoting,
            PactError::NotInVotingPhase
        );
        require!(
            participant_state.deposited,
            PactError::NotDeposited
        );

        participant_state.vote_continue = Some(wants_continue);

        Ok(())
    }
}

// ============================================================
// ACCOUNTS
// ============================================================

#[derive(Accounts)]
#[instruction(challenge_id: String)]
pub struct CreateChallenge<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init,
        payer = creator,
        space = Challenge::SPACE,
        seeds = [b"challenge", challenge_id.as_bytes()],
        bump
    )]
    pub challenge: Account<'info, Challenge>,

    /// The USDC vault for this challenge
    #[account(
        init,
        payer = creator,
        token::mint = usdc_mint,
        token::authority = challenge,
        seeds = [b"vault", challenge.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, anchor_spl::token::Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct JoinChallenge<'info> {
    #[account(mut)]
    pub participant: Signer<'info>,

    #[account(mut)]
    pub challenge: Account<'info, Challenge>,

    #[account(
        init,
        payer = participant,
        space = ParticipantState::SPACE,
        seeds = [
            b"participant",
            challenge.key().as_ref(),
            participant.key().as_ref()
        ],
        bump
    )]
    pub participant_state: Account<'info, ParticipantState>,

    #[account(
        mut,
        constraint = participant_token_account.owner == participant.key(),
        constraint = participant_token_account.mint == challenge_mint.key()
    )]
    pub participant_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", challenge.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    pub challenge_mint: Account<'info, anchor_spl::token::Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartChallenge<'info> {
    pub creator: Signer<'info>,
    #[account(mut)]
    pub challenge: Account<'info, Challenge>,
}

#[derive(Accounts)]
pub struct SubmitResult<'info> {
    pub participant: Signer<'info>,

    #[account(mut)]
    pub challenge: Account<'info, Challenge>,

    #[account(
        mut,
        seeds = [
            b"participant",
            challenge.key().as_ref(),
            participant.key().as_ref()
        ],
        bump = participant_state.bump
    )]
    pub participant_state: Account<'info, ParticipantState>,
}

#[derive(Accounts)]
pub struct Dispute<'info> {
    pub disputer: Signer<'info>,

    pub challenge: Account<'info, Challenge>,

    /// State yang di-dispute
    #[account(
        mut,
        constraint = target_participant_state.challenge == challenge.key()
    )]
    pub target_participant_state: Account<'info, ParticipantState>,

    /// State disputer (harus deposited)
    #[account(
        constraint = disputer_state.challenge == challenge.key(),
        constraint = disputer_state.deposited == true,
        seeds = [
            b"participant",
            challenge.key().as_ref(),
            disputer.key().as_ref()
        ],
        bump = disputer_state.bump
    )]
    pub disputer_state: Account<'info, ParticipantState>,
}

#[derive(Accounts)]
pub struct Finalize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub challenge: Account<'info, Challenge>,

    #[account(
        mut,
        seeds = [b"vault", challenge.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    /// CHECK: Treasury wallet
    #[account(mut)]
    pub treasury: AccountInfo<'info>,

    #[account(
        mut,
        constraint = treasury_token_account.owner == treasury.key()
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct VoteContinue<'info> {
    pub participant: Signer<'info>,
    pub challenge: Account<'info, Challenge>,
    #[account(
        mut,
        seeds = [
            b"participant",
            challenge.key().as_ref(),
            participant.key().as_ref()
        ],
        bump = participant_state.bump
    )]
    pub participant_state: Account<'info, ParticipantState>,
}

// ============================================================
// STATE
// ============================================================

#[account]
pub struct Challenge {
    pub creator: Pubkey,         // 32
    pub challenge_id: String,    // 4 + max 32
    pub title: String,           // 4 + max 64
    pub stake_amount: u64,       // 8
    pub max_participants: u8,    // 1
    pub participant_count: u8,   // 1
    pub deposit_count: u8,       // 1
    pub duration_seconds: i64,   // 8
    pub created_at: i64,         // 8
    pub deadline: i64,           // 8
    pub submission_deadline: i64, // 8
    pub status: ChallengeStatus, // 1
    pub cycle: u8,               // 1
    pub bump: u8,                // 1
    pub vault_bump: u8,          // 1
}

impl Challenge {
    // 8 (discriminator) + 32 + 36 + 68 + 8 + 1 + 1 + 1 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + padding
    pub const SPACE: usize = 8 + 32 + 36 + 68 + 8 + 1 + 1 + 1 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + 64;
}

#[account]
pub struct ParticipantState {
    pub participant: Pubkey,           // 32
    pub challenge: Pubkey,             // 32
    pub deposited: bool,               // 1
    pub submitted: bool,               // 1
    pub submission_result: SubmissionResult, // 1
    pub proof_hash: [u8; 32],          // 32
    pub disputed: bool,                // 1
    pub dispute_count: u8,             // 1
    pub is_winner: bool,               // 1
    pub vote_continue: Option<bool>,   // 2
    pub bump: u8,                      // 1
}

impl ParticipantState {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1 + 1 + 32 + 1 + 1 + 1 + 2 + 1 + 32;
}

// ============================================================
// ENUMS
// ============================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ChallengeStatus {
    Open,          // Menunggu participants
    Active,        // Sedang berjalan
    Submission,    // Deadline passed, 24hr submission window
    AllFailVoting, // Semua fail, voting continue/refund
    Settled,       // Selesai
    Cancelled,     // Dibatalkan
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum SubmissionResult {
    None,
    Success,
    Fail,
}

// ============================================================
// EVENTS
// ============================================================

#[event]
pub struct ChallengeCreated {
    pub challenge_key: Pubkey,
    pub creator: Pubkey,
    pub stake_amount: u64,
    pub title: String,
}

#[event]
pub struct ParticipantJoined {
    pub challenge_key: Pubkey,
    pub participant: Pubkey,
    pub total_participants: u8,
}

#[event]
pub struct ResultSubmitted {
    pub challenge_key: Pubkey,
    pub participant: Pubkey,
    pub success: bool,
}

#[event]
pub struct DisputeFiled {
    pub challenge_key: Pubkey,
    pub disputer: Pubkey,
    pub target: Pubkey,
    pub total_disputes: u8,
}

#[event]
pub struct ChallengeSettled {
    pub challenge_key: Pubkey,
    pub winner_count: u8,
    pub total_pool: u64,
    pub fee: u64,
}

// ============================================================
// ERRORS
// ============================================================

#[error_code]
pub enum PactError {
    #[msg("Stake amount must be > 0")]
    InvalidStake,
    #[msg("Need at least 2 participants")]
    TooFewParticipants,
    #[msg("Max 10 participants")]
    TooManyParticipants,
    #[msg("Invalid challenge duration")]
    InvalidDuration,
    #[msg("Title too long (max 64 chars)")]
    TitleTooLong,
    #[msg("Challenge is not open for joining")]
    ChallengeNotOpen,
    #[msg("Challenge is full")]
    ChallengeFull,
    #[msg("Only the creator can perform this action")]
    NotCreator,
    #[msg("Not in submission window")]
    NotInSubmissionWindow,
    #[msg("Challenge period has not ended yet")]
    ChallengeNotEnded,
    #[msg("Submission window has closed")]
    SubmissionWindowClosed,
    #[msg("Must deposit to participate")]
    NotDeposited,
    #[msg("Already submitted")]
    AlreadySubmitted,
    #[msg("Cannot dispute yourself")]
    CannotDisputeSelf,
    #[msg("Cannot finalize in current state")]
    CannotFinalize,
    #[msg("Submission window has not closed yet")]
    SubmissionWindowNotClosed,
    #[msg("Not in voting phase")]
    NotInVotingPhase,
    #[msg("Invalid number of remaining accounts")]
    InvalidAccountCount,
    #[msg("Participant does not belong to this challenge")]
    InvalidParticipant,
}