## ADDED Requirements

### Requirement: User reward accounts track aggregate lock state
The system SHALL maintain one reward account per payer user with aggregate `locked`, `released`, and `claimed` `CLAW` totals so receipt settlement and later release/claim flows can use a stable accounting surface.

#### Scenario: Initialize user reward account on first receipt
- **WHEN** a receipt settlement would credit rewards to a payer user who does not yet have a reward account
- **THEN** the program creates that user's reward account and records the locked reward amount there

### Requirement: Provider reward accounts track aggregate lock state separate from provider debt
The system SHALL maintain one reward account per provider wallet with aggregate `locked`, `released`, and `claimed` totals, and SHALL keep signed debt semantics on the provider aggregate account rather than encoding negative values inside the reward account itself.

#### Scenario: Keep provider reward totals non-negative
- **WHEN** a provider receives a challenge penalty that pushes `claw_net_position` negative
- **THEN** the negative value is tracked on the provider aggregate account and provider reward-account totals remain non-negative accounting values

### Requirement: Phase 1 lockup semantics are defined without a daily unlock executor
The system SHALL define Phase 1 rewards as initially locked for the configured lock duration with linear day-based unlock semantics, but SHALL not require the masterpool to execute daily release materialization automatically in Phase 1.

#### Scenario: Preserve locked rewards without automated release job
- **WHEN** receipt settlement credits new user or provider rewards during Phase 1
- **THEN** those rewards are added to locked balances and do not become claimable solely because time has passed unless released accounting has been materialized by an authorized future mechanism

### Requirement: Claims transfer only already released `CLAW`
The system SHALL allow the owner of a user reward account or provider reward account to claim only `released_claw_total - claimed_claw_total`, and SHALL increase `claimed_claw_total` by exactly the amount transferred.

#### Scenario: Claim available released rewards
- **WHEN** a reward-account owner invokes the claim instruction and the account has released but unclaimed `CLAW`
- **THEN** the program transfers exactly the released-but-unclaimed amount from reward inventory and updates the claimed total to match

#### Scenario: Reject claim with no released balance
- **WHEN** a reward-account owner invokes the claim instruction and `released_claw_total` is not greater than `claimed_claw_total`
- **THEN** the claim instruction fails or transfers zero without reducing reward-vault inventory
