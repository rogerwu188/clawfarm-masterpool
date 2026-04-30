# masterpool-phase1-config Specification

## Purpose
TBD - created by archiving change clawfarm-masterpool-phase1-core-economics. Update Purpose after archive.
## Requirements
### Requirement: Phase 1 masterpool initialization
The system SHALL initialize a singleton Phase 1 masterpool config that binds the admin authority, the configured attestation program, the `CLAW` mint, the `USDC` mint, and all vaults required for reward inventory, provider stake, provider-pending revenue, treasury revenue, and challenge bonds.

#### Scenario: Initialize config and vault bindings
- **WHEN** the deployer initializes the Phase 1 masterpool with valid admin, mint, vault, and attestation-program inputs
- **THEN** the program stores those bindings in the singleton config account and makes them available for later economic instructions

### Requirement: Fixed `CLAW` genesis supply
The system SHALL mint exactly `1_000_000_000 * 10^6` base units of `CLAW` one time into the masterpool reward vault and SHALL prevent any second genesis mint.

#### Scenario: Mint genesis supply once
- **WHEN** the admin executes the one-time genesis mint flow after initialization
- **THEN** the entire fixed `CLAW` supply is minted into the reward vault and the config records that genesis minting is complete

#### Scenario: Reject duplicate genesis mint
- **WHEN** any caller attempts to mint the genesis supply after the one-time mint has already completed
- **THEN** the instruction fails without increasing total supply

### Requirement: Routine rewards must not rely on long-lived mint authority
The system MUST support Phase 1 reward accounting without depending on a long-lived mint authority for routine issuance after the genesis supply is minted into the reward vault.

#### Scenario: Revoke or avoid routine mint authority usage
- **WHEN** Phase 1 reward settlement begins after the genesis mint is completed
- **THEN** reward distribution uses pre-minted reward-vault inventory rather than newly minting `CLAW`

### Requirement: Admin parameter updates enforce invariants and preserve history
The system SHALL allow only the configured admin authority to update governance parameters, SHALL enforce all configured split invariants and non-zero positive amount rules, and SHALL apply updated values only to future receipts or future challenges.

#### Scenario: Accept valid parameter update
- **WHEN** the admin submits a config update whose split ratios sum to `1000` and whose stake, bond, and slash values are positive
- **THEN** the new values are stored for future use

#### Scenario: Reject invalid parameter update
- **WHEN** any caller submits a config update whose paired split ratios do not sum to `1000` or whose positive-integer amounts are zero
- **THEN** the instruction fails and previously stored config values remain unchanged

#### Scenario: Preserve historical settlement snapshots
- **WHEN** governance changes an exchange rate, split ratio, or slash amount after receipts or challenges already exist
- **THEN** later settlement of those existing receipts or challenges uses the snapshotted historical values rather than the new config

### Requirement: Attestation program identity is configurable and enforced
The system SHALL store the authorized attestation program ID in config and SHALL reject receipt or challenge economic instructions that are not invoked by that exact configured program through CPI.

#### Scenario: Allow configured attestation CPI caller
- **WHEN** a receipt or challenge economic instruction is invoked through CPI by the configured attestation program
- **THEN** the masterpool instruction proceeds to validate the rest of the economic state transition

#### Scenario: Reject unauthorized caller
- **WHEN** any direct caller or any CPI caller other than the configured attestation program invokes an attestation-only economic instruction
- **THEN** the instruction fails without mutating settlement state or token balances

