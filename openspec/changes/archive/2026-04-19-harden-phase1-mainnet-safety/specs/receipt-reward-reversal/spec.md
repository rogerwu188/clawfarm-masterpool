## ADDED Requirements

### Requirement: Receipt-time rewards are recorded as provisional balances
The system SHALL snapshot user and provider reward amounts at receipt-record time, but SHALL store those balances as provisional reward state until the receipt becomes economically valid for final settlement.

#### Scenario: Record provisional reward balances at receipt time
- **WHEN** a new receipt is recorded successfully
- **THEN** the receipt settlement stores the receipt-specific reward snapshot and the user/provider reward accounts increase their provisional balances rather than their finalized locked balances

### Requirement: Finalized valid receipts promote provisional rewards into locked rewards
The system SHALL promote receipt-linked provisional rewards into locked reward balances only when the receipt remains economically valid through the finalized settlement path.

#### Scenario: Promote provisional balances on finalized receipt
- **WHEN** a valid receipt reaches the finalized settlement path
- **THEN** the receipt's provisional user and provider rewards move into locked reward balances and the provider-share USDC payout is released

#### Scenario: Rejected challenge keeps rewards provisional until later finalize
- **WHEN** a challenge is resolved as rejected and the receipt remains valid but not yet finalized for payout
- **THEN** the receipt's reward balances remain provisional until the later finalized settlement occurs

### Requirement: Successful challenges unwind provisional reward effects before slash
The system SHALL unwind the challenged receipt's provisional user reward, provisional provider reward, and provisional provider net-position effect before applying the snapshotted provider slash, challenger reward, burn, and payer refund economics.

#### Scenario: Successful challenge reverses receipt-booked rewards
- **WHEN** a challenge succeeds against a receipt that is still awaiting economic finalization
- **THEN** the system removes the receipt's provisional reward effects, refunds the provider-share USDC to the payer, returns the challenger bond, and only then applies the snapshotted slash, challenger reward, and burn outcomes

### Requirement: Provisional rewards are not releasable or claimable
The system SHALL compute reward release and reward claims only from finalized locked balances, and SHALL NOT allow provisional balances to become claimable through the Phase 1 release helper or claim path.

#### Scenario: Reject release sourced only from provisional balances
- **WHEN** an operator attempts to materialize a reward release that exceeds the owner's finalized locked balance but not the owner's provisional balance
- **THEN** the instruction fails without changing released or claimed totals

