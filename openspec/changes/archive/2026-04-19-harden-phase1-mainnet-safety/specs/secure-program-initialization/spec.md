## ADDED Requirements

### Requirement: Masterpool initialization is authorized by the current upgrade authority
The system SHALL allow `clawfarm-masterpool` singleton initialization only when the signer that matches the program's current ProgramData upgrade authority authorizes the initialization transaction, and SHALL reject any other first caller.

#### Scenario: Authorized bootstrap initializes masterpool
- **WHEN** the current `clawfarm-masterpool` upgrade authority signs the initialization transaction
- **THEN** the program creates the singleton config and binds the requested operational authorities and linked program IDs

#### Scenario: Arbitrary first caller is rejected for masterpool
- **WHEN** a signer that is not the current `clawfarm-masterpool` upgrade authority attempts the first initialization
- **THEN** the instruction fails and no singleton config or vault binding is created

### Requirement: Attestation initialization is authorized by the current upgrade authority
The system SHALL allow `clawfarm-attestation` singleton initialization only when the signer that matches the program's current ProgramData upgrade authority authorizes the initialization transaction, and SHALL reject any other first caller.

#### Scenario: Authorized bootstrap initializes attestation
- **WHEN** the current `clawfarm-attestation` upgrade authority signs the initialization transaction
- **THEN** the program creates the singleton config and binds the requested authority, pause authority, challenge resolver, and masterpool program

#### Scenario: Arbitrary first caller is rejected for attestation
- **WHEN** a signer that is not the current `clawfarm-attestation` upgrade authority attempts the first initialization
- **THEN** the instruction fails and no singleton config is created

