use crate::constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementBreakdown {
    pub fee: u64,
    pub penalty_pool: u64,
    pub reward_per_winner: u64,
    pub reward_remainder: u64,
    pub all_win: bool,
    pub all_fail: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreOutcome {
    Win,
    Fail,
    AutoFail,
    DisputedFail,
}

pub fn calculate_settlement(
    stake_amount: u64,
    participant_count: usize,
    winner_count: usize,
) -> SettlementBreakdown {
    let total_pool = stake_amount * participant_count as u64;
    let loser_count = participant_count.saturating_sub(winner_count);
    let penalty_pool = stake_amount * loser_count as u64;
    let all_win = winner_count == participant_count;
    let all_fail = winner_count == 0;

    if all_fail {
        let fee = total_pool * SETTLEMENT_FEE_BPS / 10_000;
        let distributable_refund = total_pool.saturating_sub(fee);
        return SettlementBreakdown {
            fee,
            penalty_pool,
            reward_per_winner: distributable_refund / participant_count as u64,
            reward_remainder: distributable_refund % participant_count as u64,
            all_win,
            all_fail,
        };
    }

    let fee = if all_win {
        0
    } else {
        penalty_pool * SETTLEMENT_FEE_BPS / 10_000
    };
    let distributable_bonus = penalty_pool.saturating_sub(fee);
    SettlementBreakdown {
        fee,
        penalty_pool,
        reward_per_winner: stake_amount + (distributable_bonus / winner_count as u64),
        reward_remainder: distributable_bonus % winner_count as u64,
        all_win,
        all_fail,
    }
}

pub fn calculate_score_delta(boost_enabled: bool, outcome: ScoreOutcome) -> i64 {
    match (boost_enabled, outcome) {
        (true, ScoreOutcome::Win) => BOOSTED_WIN_SCORE,
        (true, ScoreOutcome::Fail) => BOOSTED_FAIL_SCORE,
        (true, ScoreOutcome::AutoFail) => BOOSTED_AUTO_FAIL_SCORE,
        (true, ScoreOutcome::DisputedFail) => BOOSTED_DISPUTED_FAIL_SCORE,
        (false, ScoreOutcome::Win) => WIN_SCORE,
        (false, ScoreOutcome::Fail) => FAIL_SCORE,
        (false, ScoreOutcome::AutoFail) => AUTO_FAIL_SCORE,
        (false, ScoreOutcome::DisputedFail) => DISPUTED_FAIL_SCORE,
    }
}

pub fn clamp_commitment_score(score: i64) -> i64 {
    score.clamp(MIN_COMMITMENT_SCORE, MAX_COMMITMENT_SCORE)
}
