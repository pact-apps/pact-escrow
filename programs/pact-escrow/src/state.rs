use anchor_lang::prelude::*;

#[account]
pub struct PlatformConfig {
    pub authority: Pubkey,
    pub treasury_authority: Pubkey,
    pub bump: u8,
}

impl PlatformConfig {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 24;
}

#[account]
pub struct Challenge {
    pub creator: Pubkey,
    pub challenge_id: String,
    pub title: String,
    pub stake_amount: u64,
    pub max_participants: u8,
    pub participant_count: u8,
    pub deposit_count: u8,
    pub duration_seconds: i64,
    pub created_at: i64,
    pub deadline: i64,
    pub submission_deadline: i64,
    pub status: ChallengeStatus,
    pub cycle: u8,
    pub bump: u8,
    pub vault_bump: u8,
}

impl Challenge {
    pub const SPACE: usize = 8 + 32 + 36 + 68 + 8 + 1 + 1 + 1 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + 64;
}

#[account]
pub struct ParticipantState {
    pub participant: Pubkey,
    pub challenge: Pubkey,
    pub deposited: bool,
    pub submitted: bool,
    pub submission_result: SubmissionResult,
    pub proof_hash: [u8; 32],
    pub disputed: bool,
    pub dispute_count: u8,
    pub is_winner: bool,
    pub boost_enabled: bool,
    pub vote_continue: Option<bool>,
    pub bump: u8,
}

impl ParticipantState {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1 + 1 + 32 + 1 + 1 + 1 + 1 + 2 + 1 + 32;
}

#[account]
pub struct CommitmentProfile {
    pub owner: Pubkey,
    pub commitment_score: i64,
    pub active_challenge_count: u32,
    pub bump: u8,
}

impl CommitmentProfile {
    pub const SPACE: usize = 8 + 32 + 8 + 4 + 1 + 24;
}

#[account]
pub struct SkrLockAccount {
    pub owner: Pubkey,
    pub skr_mint: Pubkey,
    pub amount_locked: u64,
    pub lock_timestamp: i64,
    pub bump: u8,
    pub vault_bump: u8,
}

impl SkrLockAccount {
    pub const SPACE: usize = 8 + 32 + 32 + 8 + 8 + 1 + 1 + 24;
}

#[account]
pub struct DisputeReceipt {
    pub challenge: Pubkey,
    pub disputer: Pubkey,
    pub target: Pubkey,
    pub bump: u8,
}

impl DisputeReceipt {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 1 + 16;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ChallengeStatus {
    Open,
    Active,
    Submission,
    AllFailVoting,
    Settled,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum SubmissionResult {
    None,
    Success,
    Fail,
}
