use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::PactError;
use crate::state::*;

#[derive(Accounts)]
pub struct InitializePlatformConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = PlatformConfig::SPACE,
        seeds = [b"platform_config"],
        bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePlatformConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"platform_config"],
        bump = platform_config.bump,
        has_one = authority @ PactError::InvalidPlatformAuthority
    )]
    pub platform_config: Account<'info, PlatformConfig>,
}

#[derive(Accounts)]
pub struct LockSkr<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = SkrLockAccount::SPACE,
        seeds = [b"skr_lock", user.key().as_ref()],
        bump
    )]
    pub skr_lock: Account<'info, SkrLockAccount>,

    #[account(
        init_if_needed,
        payer = user,
        token::mint = skr_mint,
        token::authority = skr_lock,
        seeds = [b"skr_vault", user.key().as_ref()],
        bump
    )]
    pub skr_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = user,
        space = CommitmentProfile::SPACE,
        seeds = [b"commitment_profile", user.key().as_ref()],
        bump
    )]
    pub commitment_profile: Account<'info, CommitmentProfile>,

    #[account(
        mut,
        constraint = user_skr_token_account.owner == user.key(),
        constraint = user_skr_token_account.mint == skr_mint.key()
    )]
    pub user_skr_token_account: Account<'info, TokenAccount>,

    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnlockSkr<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"skr_lock", user.key().as_ref()],
        bump = skr_lock.bump
    )]
    pub skr_lock: Account<'info, SkrLockAccount>,

    #[account(
        mut,
        seeds = [b"skr_vault", user.key().as_ref()],
        bump = skr_lock.vault_bump
    )]
    pub skr_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"commitment_profile", user.key().as_ref()],
        bump = commitment_profile.bump
    )]
    pub commitment_profile: Account<'info, CommitmentProfile>,

    #[account(
        mut,
        constraint = user_skr_token_account.owner == user.key(),
        constraint = user_skr_token_account.mint == skr_lock.skr_mint
    )]
    pub user_skr_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

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

    #[account(
        init,
        payer = creator,
        token::mint = usdc_mint,
        token::authority = challenge,
        seeds = [b"vault", challenge.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,
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
        init_if_needed,
        payer = participant,
        space = CommitmentProfile::SPACE,
        seeds = [b"commitment_profile", participant.key().as_ref()],
        bump
    )]
    pub commitment_profile: Account<'info, CommitmentProfile>,

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

    /// CHECK: Optional SKR lock PDA. If present and valid, it enables boost for this join.
    pub skr_lock: UncheckedAccount<'info>,

    pub challenge_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
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
    #[account(mut)]
    pub disputer: Signer<'info>,

    pub challenge: Account<'info, Challenge>,

    #[account(
        mut,
        constraint = target_participant_state.challenge == challenge.key()
    )]
    pub target_participant_state: Account<'info, ParticipantState>,

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

    #[account(
        init,
        payer = disputer,
        space = DisputeReceipt::SPACE,
        seeds = [
            b"dispute",
            challenge.key().as_ref(),
            disputer.key().as_ref(),
            target_participant_state.participant.as_ref()
        ],
        bump
    )]
    pub dispute_receipt: Account<'info, DisputeReceipt>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Finalize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub challenge: Account<'info, Challenge>,

    #[account(
        mut,
        seeds = [b"vault", challenge.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.owner == platform_config.treasury_authority @ PactError::InvalidTreasuryTokenAccount,
        constraint = treasury_token_account.mint == vault.mint @ PactError::InvalidTreasuryMint
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
