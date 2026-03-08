use anchor_lang::prelude::*;

pub mod constants;
pub mod contexts;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod logic;
pub mod state;

pub use constants::*;
pub use contexts::*;
pub use errors::*;
pub use events::*;
pub use logic::*;
pub use state::*;

declare_id!("CvTggHr71Qm6NC5qjkvCq4txe2UbbZWrKAWGpMVMWw6y");

#[program]
pub mod pact_escrow {
    use super::*;

    pub fn initialize_platform_config(
        ctx: Context<InitializePlatformConfig>,
        treasury_authority: Pubkey,
    ) -> Result<()> {
        instructions::platform::initialize_platform_config(ctx, treasury_authority)
    }

    pub fn update_platform_config(
        ctx: Context<UpdatePlatformConfig>,
        treasury_authority: Pubkey,
    ) -> Result<()> {
        instructions::platform::update_platform_config(ctx, treasury_authority)
    }

    pub fn lock_skr(ctx: Context<LockSkr>, amount: u64) -> Result<()> {
        instructions::skr::lock_skr(ctx, amount)
    }

    pub fn unlock_skr(ctx: Context<UnlockSkr>, amount: u64) -> Result<()> {
        instructions::skr::unlock_skr(ctx, amount)
    }

    pub fn create_challenge(
        ctx: Context<CreateChallenge>,
        challenge_id: String,
        stake_amount: u64,
        duration_seconds: i64,
        max_participants: u8,
        title: String,
    ) -> Result<()> {
        instructions::challenge::create_challenge(
            ctx,
            challenge_id,
            stake_amount,
            duration_seconds,
            max_participants,
            title,
        )
    }

    pub fn join_challenge(ctx: Context<JoinChallenge>) -> Result<()> {
        instructions::challenge::join_challenge(ctx)
    }

    pub fn start_challenge(ctx: Context<StartChallenge>) -> Result<()> {
        instructions::challenge::start_challenge(ctx)
    }

    pub fn submit_result(
        ctx: Context<SubmitResult>,
        success: bool,
        proof_hash: [u8; 32],
    ) -> Result<()> {
        instructions::challenge::submit_result(ctx, success, proof_hash)
    }

    pub fn dispute(ctx: Context<Dispute>) -> Result<()> {
        instructions::challenge::dispute(ctx)
    }

    pub fn finalize<'info>(ctx: Context<'_, '_, 'info, 'info, Finalize<'info>>) -> Result<()> {
        instructions::settlement::finalize(ctx)
    }

    pub fn vote_continue(ctx: Context<VoteContinue>, wants_continue: bool) -> Result<()> {
        instructions::challenge::vote_continue(ctx, wants_continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_with_winners_only_fees_loser_penalty() {
        let settlement = calculate_settlement(100, 4, 2);

        assert_eq!(
            settlement,
            SettlementBreakdown {
                fee: 10,
                penalty_pool: 200,
                reward_per_winner: 195,
                reward_remainder: 0,
                all_win: false,
                all_fail: false,
            }
        );
    }

    #[test]
    fn settlement_all_win_refunds_full_stake_without_fee() {
        let settlement = calculate_settlement(100, 4, 4);

        assert_eq!(
            settlement,
            SettlementBreakdown {
                fee: 0,
                penalty_pool: 0,
                reward_per_winner: 100,
                reward_remainder: 0,
                all_win: true,
                all_fail: false,
            }
        );
    }

    #[test]
    fn settlement_all_fail_refunds_equally_after_fee() {
        let settlement = calculate_settlement(100, 4, 0);

        assert_eq!(
            settlement,
            SettlementBreakdown {
                fee: 20,
                penalty_pool: 400,
                reward_per_winner: 95,
                reward_remainder: 0,
                all_win: false,
                all_fail: true,
            }
        );
    }

    #[test]
    fn settlement_distributes_remainder_deterministically() {
        let settlement = calculate_settlement(100, 3, 2);

        assert_eq!(settlement.fee, 5);
        assert_eq!(settlement.penalty_pool, 100);
        assert_eq!(settlement.reward_per_winner, 147);
        assert_eq!(settlement.reward_remainder, 1);
    }

    #[test]
    fn boosted_score_changes_are_applied_only_to_reputation() {
        assert_eq!(calculate_score_delta(false, ScoreOutcome::Win), WIN_SCORE);
        assert_eq!(calculate_score_delta(true, ScoreOutcome::Win), BOOSTED_WIN_SCORE);
        assert_eq!(calculate_score_delta(false, ScoreOutcome::Fail), FAIL_SCORE);
        assert_eq!(calculate_score_delta(true, ScoreOutcome::Fail), BOOSTED_FAIL_SCORE);
        assert_eq!(
            calculate_score_delta(false, ScoreOutcome::AutoFail),
            AUTO_FAIL_SCORE
        );
        assert_eq!(
            calculate_score_delta(true, ScoreOutcome::AutoFail),
            BOOSTED_AUTO_FAIL_SCORE
        );
        assert_eq!(
            calculate_score_delta(false, ScoreOutcome::DisputedFail),
            DISPUTED_FAIL_SCORE
        );
        assert_eq!(
            calculate_score_delta(true, ScoreOutcome::DisputedFail),
            BOOSTED_DISPUTED_FAIL_SCORE
        );
    }

    #[test]
    fn commitment_score_is_clamped_between_zero_and_hundred() {
        assert_eq!(clamp_commitment_score(-25), 0);
        assert_eq!(clamp_commitment_score(45), 45);
        assert_eq!(clamp_commitment_score(145), 100);
    }

    #[test]
    fn platform_config_tracks_fixed_treasury_authority() {
        let authority = Pubkey::new_unique();
        let treasury_authority = Pubkey::new_unique();
        let config = PlatformConfig {
            authority,
            treasury_authority,
            bump: 254,
        };

        assert_eq!(config.authority, authority);
        assert_eq!(config.treasury_authority, treasury_authority);
    }
}
