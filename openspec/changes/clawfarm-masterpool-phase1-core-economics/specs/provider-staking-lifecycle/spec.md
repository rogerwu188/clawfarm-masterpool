## ADDED Requirements

### Requirement: Provider identity is bound to one wallet
The system SHALL represent each provider by exactly one primary wallet address in Phase 1 and SHALL derive provider state from that wallet identity.

#### Scenario: Derive provider account from provider wallet
- **WHEN** a provider registers with a wallet that does not yet have a provider account
- **THEN** the program creates provider state keyed to that wallet and treats it as the provider's sole Phase 1 on-chain identity

### Requirement: Provider registration requires USDC stake
The system SHALL require a provider to transfer the configured USDC stake amount during registration and SHALL initialize both provider aggregate accounting and provider reward accounting at that time.

#### Scenario: Register provider with full stake
- **WHEN** a provider wallet submits registration with at least the configured stake amount available
- **THEN** the program transfers the configured USDC stake into the provider stake vault and creates the provider account and provider reward account

#### Scenario: Reject underfunded registration
- **WHEN** a provider wallet attempts registration without funding the full configured USDC stake amount
- **THEN** the registration fails and no provider account becomes active

### Requirement: Provider state tracks exit-blocking obligations
The system SHALL track provider pending revenue, unsettled receipt count, and signed `CLAW` net position so later instructions can determine whether the provider still has unresolved obligations.

#### Scenario: Increase unsettled obligations after receipt recording
- **WHEN** a receipt is recorded for a provider through attestation CPI
- **THEN** the provider account reflects the new pending provider revenue and unsettled receipt count associated with that provider

### Requirement: Provider exit is blocked until obligations are cleared
The system SHALL allow provider exit only when pending provider USDC is zero, unsettled receipt count is zero, `claw_net_position` is non-negative, and no unresolved challenge economics remain.

#### Scenario: Exit provider after obligations are cleared
- **WHEN** a provider with zero pending provider USDC, zero unsettled receipts, no unresolved challenge record, and non-negative `CLAW` net position requests exit
- **THEN** the program returns the provider's stake and marks the provider inactive or exited

#### Scenario: Reject provider exit with unresolved obligations
- **WHEN** a provider requests exit while any pending provider USDC, unsettled receipt, unresolved challenge, or negative `CLAW` net position remains
- **THEN** the exit instruction fails and the provider stake stays locked
