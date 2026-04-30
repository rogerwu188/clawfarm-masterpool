## ADDED Requirements

### Requirement: Attestation config defines a bounded challenge-resolution timeout
The system SHALL store a positive `challenge_resolution_timeout_seconds` value in attestation configuration and SHALL use it to determine when an open challenge has exceeded its normal resolution window.

#### Scenario: Initialize positive challenge-resolution timeout
- **WHEN** attestation configuration is initialized for mainnet operation
- **THEN** it stores a positive timeout value that can later be used to evaluate whether an open challenge is stale

### Requirement: Authority may timeout-reject a stale open challenge
The system SHALL provide a dedicated timeout fallback that allows attestation authority to reject an open challenge after its resolution timeout has elapsed when the normal challenge-resolver path has not completed.

#### Scenario: Timeout-reject stale challenge
- **WHEN** a challenge remains `Open` after `opened_at + challenge_resolution_timeout_seconds` and attestation authority invokes the timeout fallback
- **THEN** the challenge is marked `Rejected` through the timeout path and masterpool processes the rejected-challenge bond burn flow

#### Scenario: Reject premature timeout resolution
- **WHEN** attestation authority invokes the timeout fallback before the challenge-resolution timeout has elapsed
- **THEN** the instruction fails and the challenge remains `Open`

### Requirement: Timeout rejection preserves the existing finalized-receipt path
The system SHALL leave a timeout-rejected challenge's receipt eligible for the normal finalized-settlement flow instead of forcing a separate payout path.

#### Scenario: Finalize receipt after timeout rejection
- **WHEN** a receipt's challenge has been timeout-rejected and the receipt later reaches the finalized settlement path
- **THEN** the existing finalized-settlement instruction can release provider-share USDC and promote provisional rewards without requiring a special-case payout flow

