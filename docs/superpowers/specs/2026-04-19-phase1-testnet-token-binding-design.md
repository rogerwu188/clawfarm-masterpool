# Phase 1 Testnet Token Binding Design

Status: Draft for review
Date: 2026-04-19
Scope: Testnet asset issuance and mint-binding rules for the current Phase 1 contracts
Related contracts:
- `programs/clawfarm-masterpool`
- `programs/clawfarm-attestation`

## 1. Goal

Define a testnet asset model that lets the existing Phase 1 contracts run with
fixed, known token addresses instead of accepting arbitrary SPL tokens.

The design must satisfy four goals:

- issue a dedicated `CLAW` mint for protocol rewards on testnet
- issue a dedicated test settlement mint that simulates `USDC` on testnet
- bind both mint addresses into the deployed `masterpool` config so every
  payment, stake, reward, and challenge flow uses the same asset pair
- keep test stablecoin minting outside the contracts so faucet and operations
  stay flexible

## 2. Chosen Design

Phase 1 testnet will use exactly two protocol-recognized mints:

- `CLAW`
  - dedicated testnet reward token
  - `6` decimals
  - fixed genesis supply target of `1_000_000_000` tokens
  - minted once into the protocol reward vault through
    `masterpool::mint_genesis_supply`
  - mint authority and freeze authority are revoked immediately after genesis
- `Test USDC`
  - dedicated testnet payment token that simulates `USDC`
  - `6` decimals
  - mint address is fixed in contract config
  - minting remains fully off-chain under a separate operator wallet
  - supply is minted on demand for testing instead of by contract logic

The `masterpool` config becomes the single on-chain source of truth for the
approved asset pair:

- `config.claw_mint`
- `config.usdc_mint`

No post-deployment mint rotation path is added in Phase 1. Once the config is
initialized, the deployed environment is permanently tied to that `CLAW` mint
and that `Test USDC` mint.

## 3. Why This Fits the Current Contracts

The current Phase 1 implementation already enforces the core model needed for
this design:

- `initialize_masterpool` stores `claw_mint` and `usdc_mint` in
  `GlobalConfig`
- Phase 1 token decimals are already constrained to `6` for both mints
- receipt settlement checks `charge_mint == config.usdc_mint`
- provider registration / exit, receipt settlement, challenge settlement, and
  reward claiming all validate token accounts against the configured mint
- `mint_genesis_supply` already mints the fixed `CLAW` genesis inventory into
  the reward vault and then revokes mint and freeze authority

Because those checks already exist, the design does not need a new generalized
token allowlist. It only needs disciplined deployment and environment
configuration around the existing single-pair mint binding.

## 4. Asset Responsibilities

### 4.1 `CLAW`

`CLAW` is a protocol-controlled reward asset, not just a generic external test
token.

Responsibilities:

- fund user reward payouts
- fund challenger reward payouts
- absorb burn flows in rejected or accepted challenge outcomes
- provide a known and auditable reward inventory on testnet

Rules:

- total target supply is exactly `1_000_000_000 * 10^6` base units
- the supply is created only through the existing genesis flow
- routine rewards must draw from `reward_vault`, not from ongoing minting
- there must be no surviving mint authority after genesis completion

### 4.2 `Test USDC`

`Test USDC` is an environment payment asset, not a protocol-controlled reward
asset.

Responsibilities:

- provider staking
- payer settlement for receipt charges
- treasury revenue accounting
- provider pending-revenue escrow and final release

Rules:

- the mint address is fixed in `GlobalConfig`
- the mint authority is not transferred to the contracts
- minting is performed only by an external operator wallet
- frontends and scripts must label it as `Test USDC` or `Mock USDC`, not as the
  canonical production `USDC`

The operational intention remains a testnet issuance budget of
`1_000_000_000` tokens, but the contract does not enforce a hard cap because
minting stays outside the protocol. The operator wallet can mint incrementally
as test demand appears.

## 5. Trust and Permission Model

The two assets intentionally use different authority models.

### `CLAW` authority model

- initial mint creation can be done by deployment tooling
- before `mint_genesis_supply` is called, `mint authority` and
  `freeze authority` must both point to the `masterpool` `pool_authority` PDA
- `mint_genesis_supply` mints the full genesis inventory into `reward_vault`
- the same instruction then revokes both authorities
- after that point, no wallet and no contract instruction can mint more `CLAW`

### `Test USDC` authority model

- mint authority belongs to a dedicated operator wallet
- the operator wallet must be separate from the protocol admin wallet
- the contracts never receive test stablecoin mint authority
- faucet, airdrop, and test-user funding remain purely operational workflows

This split keeps protocol economics strict while preserving testnet flexibility.

## 6. Deployment Sequence

The deployment runbook for testnet should follow this order.

1. Deploy `clawfarm-masterpool` and `clawfarm-attestation`.
2. Create the `CLAW` mint with `6` decimals.
3. Derive the `masterpool` `pool_authority` PDA.
4. Transfer the `CLAW` mint's mint authority and freeze authority to
   `pool_authority`.
5. Create the `Test USDC` mint with `6` decimals.
6. Assign the `Test USDC` mint authority to a dedicated operator wallet.
7. Initialize `masterpool` with:
   - the fixed `CLAW` mint address
   - the fixed `Test USDC` mint address
   - the deployed attestation program id
   - the Phase 1 economic parameters
8. Call `mint_genesis_supply` to mint `1_000_000_000` `CLAW` into
   `reward_vault`.
9. Initialize `attestation`.
10. Use the external operator wallet to mint `Test USDC` to test users,
    providers, and any faucet inventory accounts.

Recommended verification after deployment:

- read `GlobalConfig` and confirm both mint addresses match the intended pair
- confirm the `CLAW` mint has no mint authority and no freeze authority
- confirm `reward_vault` received the full genesis supply
- confirm `Test USDC` mint authority is the external operator wallet, not the
  protocol admin and not any program PDA

## 7. Contract-Level Invariants

The deployed environment must preserve the following invariants.

### Mint invariants

- `config.claw_mint` is the only valid reward and challenge-bond mint
- `config.usdc_mint` is the only valid stake and settlement mint
- both mints use `6` decimals
- no instruction accepts a substitute mint even if the token symbol looks
  similar

### Supply invariants

- `CLAW` genesis is minted once and only once
- `CLAW` cannot be inflated after genesis
- `Test USDC` may be minted repeatedly, but only by the external operator
  wallet

### Environment invariants

- there is exactly one approved asset pair per deployment
- deployment scripts, frontend config, and test scripts must all use the same
  mint addresses
- no admin API is added to swap mint addresses after initialization

## 8. Error Handling and Failure Expectations

The environment should fail closed in the following situations:

- if either mint is not `6` decimals, `initialize_masterpool` must fail
- if a caller passes a token account whose mint does not match
  `config.claw_mint` or `config.usdc_mint`, the protocol instruction must fail
- if `CLAW` mint authority was not transferred to `pool_authority` before
  `mint_genesis_supply`, the genesis call must fail
- if someone tries to re-run `mint_genesis_supply`, the call must fail because
  `genesis_minted == true`

Operational failures outside the contracts should also be treated explicitly:

- if the `Test USDC` operator wallet is lost, testnet payment issuance stops
  until the mint authority is recovered or migrated operationally
- if frontend or scripts point at the wrong mint addresses, users will hit
  contract rejections instead of silently settling in the wrong asset

## 9. Testing Requirements

The current integration suite already proves most of the contract behavior for a
fixed mint pair. Testnet rollout should additionally verify:

- `GlobalConfig` persists the intended `CLAW` and `Test USDC` mint addresses
- `mint_genesis_supply` succeeds only after `CLAW` mint authority is transferred
  to `pool_authority`
- all key instructions reject token accounts or charge mints that do not match
  config
- the external operator can mint `Test USDC` to arbitrary test accounts without
  any contract changes
- no post-genesis `CLAW` minting remains possible

## 10. Non-Goals

This design does not add:

- a production `USDC` integration path
- a generic multi-token settlement model
- an on-chain faucet for test stablecoins
- a post-deployment admin function for rotating mint addresses
- a second `CLAW` issuance round

## 11. Recommendation

Adopt this model as the standard Phase 1 testnet asset setup:

- fixed `CLAW` mint bound in config
- fixed `Test USDC` mint bound in config
- one-time protocol-controlled `CLAW` genesis
- externally operated `Test USDC` minting

This gives the testnet the safety property you want: the contracts no longer
operate over arbitrary SPL tokens, but they also do not absorb unnecessary
faucet or off-chain issuance responsibilities.
