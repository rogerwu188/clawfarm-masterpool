## ADDED Requirements

### Requirement: Only active providers may receive new receipt economics
The system SHALL accept receipt-driven economic recording only for providers whose status is `Active`, and SHALL reject receipt settlement for providers whose provider state is `Exited` or otherwise inactive.

#### Scenario: Record receipt for active provider
- **WHEN** the attestation program records a receipt for a provider whose status is `Active`
- **THEN** the receipt economics are processed subject to the remaining receipt-settlement validations

#### Scenario: Reject receipt for exited provider
- **WHEN** the attestation program records a receipt for a provider whose status is `Exited`
- **THEN** the instruction fails without charging the payer, changing provider accounting, or creating a receipt settlement snapshot

### Requirement: Provider exit is terminal in Phase 1
The system SHALL treat the current provider PDA as terminal after exit and SHALL NOT allow the exited provider state to resume earning through the existing registration or receipt-recording path.

#### Scenario: Exited provider cannot earn again through existing state
- **WHEN** an exited provider attempts to receive new receipts using the same provider PDA
- **THEN** the program rejects the receipt because the provider is no longer active

#### Scenario: Exited provider cannot silently re-register
- **WHEN** an exited provider attempts to call the existing provider registration flow again for the same provider wallet
- **THEN** the registration does not create a new active provider state for that wallet

