# PACT Escrow

PACT Escrow is a Solana Anchor program for accountability challenges with SPL-token staking, deterministic settlement, and reputation tracking.

This repo contains the on-chain program and tests. It does not include a production frontend or backend.

## Overview

PACT supports group challenges where participants deposit stake, submit a final result, and settle funds based on challenge outcomes.

Core ideas:
- participants stake an SPL token
- winners recover principal and receive bonus from losing stake
- platform fee is enforced by the contract treasury config
- reputation is tracked with Commitment Score
- SKR acts only as a reputation booster, not as a payout modifier

## Use Cases

PACT is designed for:
- habit challenges
- accountability groups
- fitness or learning sprints
- commitment challenges with stake and reputation

Examples:
- 30-day workout challenge
- coding streak challenge
- study sprint
- finish-a-course challenge

## Current Program

- Network: `devnet`
- Program ID: `CvTggHr71Qm6NC5qjkvCq4txe2UbbZWrKAWGpMVMWw6y`
- Treasury authority: `5UBYd69pmTayz8sKDWkhduLojrJe513qr1zHBoXet228`

## Main Flow

1. Creator creates a challenge.
2. Participants join and deposit stake into the vault.
3. Challenge runs until deadline.
4. After deadline, participants submit a final result.
5. Other participants may dispute.
6. Anyone can finalize after the submission window closes.
7. Funds are distributed and Commitment Score is updated.

## Proof Model

For MVP, proof is final-result based:
- official proof is submitted at the end of the challenge
- on-chain submission stores only `proof_hash`
- actual files and daily evidence are expected to live off-chain

Recommended product model:
- `final_only`: one final proof at the end
- `daily_checkin`: daily progress lives in backend/app, but on-chain still uses one final submission

## Settlement Logic

Current payout behavior:

If there are winners:
- winners get their original stake back in full
- losing stake becomes the penalty pool
- platform fee is charged from the penalty pool
- the remaining penalty pool is split across winners
- losers receive `0`

If everyone wins:
- everyone gets a full refund
- fee is `0`

If everyone fails:
- refunds are distributed equally after the standard fee

Current settlement fee:
- `5%`

## Treasury Enforcement

Platform fee destination is no longer arbitrary from the client.

The program now uses `PlatformConfig`:
- treasury authority is stored on-chain
- `finalize()` requires a `treasury_token_account`
- that token account must:
  - be owned by the configured treasury authority
  - match the stake mint used by the challenge

Important:
- fee does not go directly to a wallet address like SOL
- fee goes to the treasury SPL token account owned by the treasury wallet

## SKR and Commitment Score

PACT includes optional SKR-based score boosting.

### SKR

Users may lock SKR before joining a challenge:
- if locked SKR is at least `100`, boost becomes active
- boost is recorded as `boost_enabled` on the participant state for that challenge

SKR does not change:
- stake amount
- payout logic
- fee logic
- dispute logic

SKR only changes Commitment Score deltas.

### Commitment Score

Commitment Score:
- default: `100`
- min: `0`
- max: `100`

Base outcomes:
- WIN = `+10`
- FAIL = `-10`
- AUTO_FAIL = `-15`
- DISPUTED_FAIL = `-20`

Boosted outcomes:
- WIN = `+15`
- FAIL = `-5`
- AUTO_FAIL = `-10`
- DISPUTED_FAIL = `-15`

## Security Notes

The program includes the following guards:
- duplicate disputes are prevented by `DisputeReceipt`
- payout token accounts are validated against participant ownership and mint
- treasury token account is validated against treasury ownership and mint
- frontend cannot freely redirect platform fees anymore

## Program Structure

The program is modularized into:
- `programs/pact-escrow/src/lib.rs`
- `programs/pact-escrow/src/constants.rs`
- `programs/pact-escrow/src/contexts.rs`
- `programs/pact-escrow/src/errors.rs`
- `programs/pact-escrow/src/events.rs`
- `programs/pact-escrow/src/logic.rs`
- `programs/pact-escrow/src/state.rs`
- `programs/pact-escrow/src/instructions/`

## Frontend / Backend Responsibilities

### On-chain

The contract is responsible for:
- escrow vault logic
- join / submit / dispute / finalize
- SKR lock state
- Commitment Score state
- treasury enforcement

### Off-chain

Frontend/backend are expected to handle:
- challenge type metadata
- daily check-ins
- proof file storage
- proof viewing and dispute UX
- treasury token account config by stake mint
- transaction building

## Finalize Account Layout

`finalize()` requires:
- `platform_config`
- `challenge`
- `vault`
- `treasury_token_account`
- `token_program`

`remaining_accounts` must be passed in strict triples:
- `ParticipantState`
- payout token account
- `CommitmentProfile`

## Build and Test

Build:

```bash
anchor build
```

Test:

```bash
cargo test
```

## Artifacts

Latest IDL:
- `target/idl/pact_escrow.json`

Latest generated TS types:
- `target/types/pact_escrow.ts`

## Notes

- The program uses SPL tokens, not native SOL, for stake.
- USDC-style staking is the intended model.
- This repo is the contract source of truth. Any frontend or backend integration should use the latest IDL from `target/idl`.
