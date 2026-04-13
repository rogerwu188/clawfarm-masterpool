# ClawFarm Phase 1 Economics

This repository now implements the Phase 1 receipt-driven economic model for
ClawFarm on Solana.

The previous epoch-settlement masterpool flow has been removed from the active
design. `clawfarm-masterpool` is now the protocol economic authority, while
`clawfarm-attestation` is the receipt and challenge lifecycle authority that
invokes masterpool through CPI.

## Programs

- `clawfarm-masterpool`
  - owns the reward, treasury, provider stake, provider pending-revenue, and
    challenge-bond vaults
  - tracks provider registration, reward balances, receipt settlements, and
    challenge bond records
  - mints the fixed `1_000_000_000 * 10^6` `CLAW` genesis supply once into the
    reward vault
- `clawfarm-attestation`
  - verifies signed receipts and signer registry membership
  - maintains receipt and challenge state transitions
  - forwards receipt recording, finalized settlement, challenge-bond recording,
    and challenge-economics resolution to masterpool through CPI

## Phase 1 Rules

- One provider equals one wallet and must stake USDC to register.
- User payments are charged per receipt, not per epoch.
- User-paid USDC is split between provider escrow and treasury at record time.
- User and provider `CLAW` rewards are snapshotted and booked as locked balances.
- Provider `CLAW` penalties are tracked as a signed net position and future
  provider rewards offset negative balance before new locked rewards are added.
- Provider USDC is released only after attestation marks the receipt finalized.
- Challenge bonds are funded in `CLAW`, not lamports.
- Rejected challenges burn the challenger's bond.
- Accepted challenges return the bond, refund the payer's provider-share USDC,
  slash the provider's signed `CLAW` position, transfer the challenger's slash
  reward from inventory, and burn the remainder.
- Phase 1 defines locked, released, and claimed reward accounting but does not
  include an automated daily unlock executor.

## Main Accounts

- `GlobalConfig`
- `ProviderAccount`
- `RewardAccount` for both users and providers
- `ReceiptSettlement`
- `ChallengeBondRecord`

See [docs/phase1-core-economics.md](docs/phase1-core-economics.md) for the full
account layout, instruction set, and state-machine flow.

## Development

```bash
anchor build
anchor test
```
