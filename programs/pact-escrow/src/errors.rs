use anchor_lang::prelude::*;

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
    #[msg("Payout account owner does not match participant")]
    InvalidPayoutAccount,
    #[msg("Payout account mint does not match challenge vault mint")]
    InvalidPayoutMint,
    #[msg("Invalid SKR lock amount")]
    InvalidSkrLockAmount,
    #[msg("Invalid SKR unlock amount")]
    InvalidSkrUnlockAmount,
    #[msg("Locked SKR balance is insufficient")]
    InsufficientSkrBalance,
    #[msg("Minimum SKR lock threshold not met")]
    InsufficientSkrLock,
    #[msg("Cannot unlock SKR while participating in an active challenge")]
    ActiveChallengeParticipation,
    #[msg("Invalid SKR lock account")]
    InvalidSkrLockAccount,
    #[msg("Invalid SKR lock owner")]
    InvalidSkrLockOwner,
    #[msg("Invalid SKR mint")]
    InvalidSkrMint,
    #[msg("Invalid commitment profile account")]
    InvalidCommitmentProfile,
    #[msg("Only the platform authority may update this config")]
    InvalidPlatformAuthority,
    #[msg("Treasury token account must belong to the configured PACT treasury authority")]
    InvalidTreasuryTokenAccount,
    #[msg("Treasury token account mint must match the challenge vault mint")]
    InvalidTreasuryMint,
}
