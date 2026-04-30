## ADDED Requirements

### Requirement: Split invariants use checked arithmetic
The system SHALL validate basis-point split invariants with checked arithmetic and SHALL reject any parameter set whose split fields overflow or do not sum to the required scale.

#### Scenario: Reject overflowed split validation
- **WHEN** a config initialization or update supplies split values whose intermediate addition would overflow the configured integer type
- **THEN** the instruction fails instead of wrapping the values into a false invariant match

#### Scenario: Reject incorrect split total
- **WHEN** a config initialization or update supplies split values that do not sum to the required `bps1000` scale
- **THEN** the instruction fails and the config remains unchanged

### Requirement: Governance parameters must fit all settlement arithmetic
The system SHALL reject any config initialization or update whose exchange rate, slash amount, bond amount, or related economic parameters would overflow any stored or derived settlement value under valid instruction execution.

#### Scenario: Reject config that cannot fit settlement math
- **WHEN** a proposed config would cause receipt settlement, challenge settlement, or signed provider accounting to overflow its supported arithmetic domain
- **THEN** the config instruction fails before any on-chain state is updated

### Requirement: Signed provider reward accounting must fail closed on overflow
The system SHALL store and update signed provider reward position in a non-wrapping domain and SHALL fail the instruction instead of wrapping whenever a provisional reward, reward reversal, or slash update exceeds that domain.

#### Scenario: Reject signed provider position overflow
- **WHEN** a receipt or challenge instruction would push the provider's signed reward position outside the supported range
- **THEN** the instruction fails without mutating provider accounting or transferring funds

