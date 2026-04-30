# receipt-settlement Specification

## Purpose
TBD - created by archiving change clawfarm-masterpool-phase1-core-economics. Update Purpose after archive.
## Requirements
### Requirement: Receipt economics are recorded only by attestation CPI
The system SHALL expose a receipt-recording instruction callable only by the configured attestation program through CPI, and that instruction SHALL require the attestation receipt identity, payer user, provider wallet, total USDC paid, and enough uniqueness data to anchor a single settlement record.

#### Scenario: Record mining from attested receipt
- **WHEN** the configured attestation program invokes receipt recording for a new valid receipt
- **THEN** the masterpool records economic state for that receipt exactly once

#### Scenario: Reject duplicate receipt settlement anchor
- **WHEN** the configured attestation program attempts to record the same receipt a second time
- **THEN** the instruction fails without creating a second settlement record or charging the payer again

### Requirement: Receipt recording charges payer, splits USDC, and snapshots economics
The system SHALL charge USDC from the receipt-bound payer user, SHALL split the payment into provider-share and treasury-share amounts using the config active at record time, SHALL transfer the treasury share into the treasury vault, SHALL escrow the provider share into pending provider revenue, and SHALL store all resulting amounts in a per-receipt settlement snapshot.

#### Scenario: Snapshot payment split at record time
- **WHEN** a receipt is recorded while the provider and treasury USDC shares are configured to specific `bps1000` values
- **THEN** the settlement snapshot stores the exact payer, provider, total paid, provider-share amount, and treasury-share amount derived from those values

### Requirement: Receipt recording credits locked rewards using snapshotted exchange rules
The system SHALL compute user and provider `CLAW` rewards from the configured exchange rate and split ratios active at receipt-record time, SHALL credit the user reward into user locked reward accounting, and SHALL apply the provider reward to any negative provider `CLAW` net position before sending any remaining positive amount into provider locked reward accounting.

#### Scenario: Credit user and provider rewards with provider debt offset
- **WHEN** a receipt is recorded for a provider whose `claw_net_position` is negative
- **THEN** the provider reward first reduces the negative position and only the remaining positive amount, if any, is added to locked provider rewards

### Requirement: Only finalized receipts can release provider USDC
The system SHALL expose a dedicated finalized-settlement instruction callable only by the configured attestation program through CPI, and it SHALL release escrowed provider USDC only when the referenced attestation receipt is economically recognized as `Finalized` and the settlement state is still `Recorded`.

#### Scenario: Release escrow after final recognition
- **WHEN** the attestation program invokes finalized settlement for a receipt whose attestation state is `Finalized` and whose settlement state is `Recorded`
- **THEN** the program transfers the snapshotted provider-share USDC to the provider wallet, reduces pending provider USDC, marks the settlement `FinalizedSettled`, and decrements the provider's unsettled receipt count

#### Scenario: Reject payout for rejected or slashed receipt
- **WHEN** the attestation program attempts finalized settlement for a receipt whose attestation state is `Rejected` or `Slashed`
- **THEN** the instruction fails and the provider payout does not occur

### Requirement: Finalized settlement is distinct from receipt cleanup
The system MUST key provider payout off a dedicated attestation-to-masterpool settlement signal and MUST NOT treat receipt cleanup or close operations as authorization to release provider USDC.

#### Scenario: Ignore receipt close as payout trigger
- **WHEN** a receipt cleanup or close action occurs without the dedicated finalized-settlement CPI call
- **THEN** the provider-share USDC remains escrowed and settlement status does not change to `FinalizedSettled`

