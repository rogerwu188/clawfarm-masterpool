# challenge-economics Specification

## Purpose
TBD - created by archiving change clawfarm-masterpool-phase1-core-economics. Update Purpose after archive.
## Requirements
### Requirement: Challenge bonds are recorded as `CLAW` collateral by attestation CPI
The system SHALL expose a challenge-bond recording instruction callable only by the configured attestation program through CPI, SHALL transfer the configured fixed `CLAW` bond from the challenger into a dedicated challenge-bond vault, and SHALL create exactly one challenge-bond record for each attestation challenge identity.

#### Scenario: Record challenge bond once
- **WHEN** the configured attestation program invokes challenge-bond recording for a new challenge
- **THEN** the program transfers the full configured `CLAW` bond into the challenge-bond vault and creates a `Locked` challenge-bond record for that challenge

#### Scenario: Reject underfunded challenge bond
- **WHEN** challenge-bond recording is attempted without the challenger funding the full configured `CLAW` bond amount
- **THEN** the instruction fails and the challenge-bond record is not created

### Requirement: Challenge-bond records snapshot penalty split parameters
The system SHALL snapshot the provider slash amount, challenger reward split, and burn split into the challenge-bond record at challenge-open time so later resolution does not depend on the current config.

#### Scenario: Preserve challenge-open ratios across config changes
- **WHEN** governance updates slash or burn parameters after a challenge bond has already been recorded
- **THEN** resolution of that existing challenge uses the values snapshotted in its challenge-bond record

### Requirement: Rejected challenges burn the challenger bond and keep receipt eligible for later finalization
The system SHALL burn the recorded challenge bond when attestation resolves the challenge as rejected, SHALL mark the bond record `Burned`, and SHALL leave the receipt settlement in `Recorded` state so it can still be finalized later if attestation marks the receipt `Finalized`.

#### Scenario: Resolve rejected challenge
- **WHEN** the configured attestation program invokes challenge-economics resolution for a recorded challenge whose attestation outcome is rejected
- **THEN** the masterpool burns the challenger's bond, marks the challenge-bond record `Burned`, and keeps the receipt settlement eligible for later finalized settlement

### Requirement: Successful challenges return bond, penalize provider, and refund payer
The system SHALL return the challenger bond when attestation resolves the challenge as accepted or otherwise invalidating, SHALL reduce the provider's signed `CLAW` net position by the snapshotted slash amount, SHALL split that slash result between challenger reward and burn using the snapshotted ratio, SHALL refund the receipt's provider-share USDC to the receipt-bound payer user, and SHALL mark the receipt settlement `ChallengedReverted`.

#### Scenario: Resolve accepted challenge before provider payout
- **WHEN** the configured attestation program resolves a challenge against a receipt whose settlement state is `Recorded`
- **THEN** the challenger bond is returned, the provider slash is applied, the challenger receives the configured reward share, the burn share is destroyed, the payer user receives the snapshotted provider-share USDC refund, and the receipt settlement becomes `ChallengedReverted`

### Requirement: Successful challenges permanently block later provider payout for the same receipt
The system SHALL forbid finalized provider payout for any receipt whose settlement has already transitioned to `ChallengedReverted`, and SHALL not allow a previously `FinalizedSettled` receipt to transition into a refunded or reverted state.

#### Scenario: Reject finalized payout after challenge-driven revert
- **WHEN** finalized settlement is attempted for a receipt whose settlement status is `ChallengedReverted`
- **THEN** the payout instruction fails without transferring provider USDC

#### Scenario: Reject late challenge economics after provider payout
- **WHEN** challenge-economics resolution is attempted for a receipt whose settlement status is already `FinalizedSettled`
- **THEN** the instruction fails without refunding USDC or changing the settled receipt state

