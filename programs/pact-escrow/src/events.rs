use anchor_lang::prelude::*;

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
    pub boost_enabled: bool,
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
    pub penalty_pool: u64,
    pub reward_per_winner: u64,
    pub reward_remainder: u64,
    pub all_win: bool,
    pub all_fail: bool,
}

#[event]
pub struct SkrLocked {
    pub owner: Pubkey,
    pub amount_locked: u64,
}

#[event]
pub struct SkrUnlocked {
    pub owner: Pubkey,
    pub amount_locked: u64,
}

#[event]
pub struct CommitmentScoreUpdated {
    pub owner: Pubkey,
    pub challenge_key: Pubkey,
    pub delta: i64,
    pub new_score: i64,
    pub boost_enabled: bool,
}

#[event]
pub struct PlatformConfigUpdated {
    pub authority: Pubkey,
    pub treasury_authority: Pubkey,
}
