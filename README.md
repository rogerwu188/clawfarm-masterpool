# ClawFarm Master Pool

Program-controlled vault for Genesis emission and protocol-controlled distribution on Solana.

## Architecture

```
┌─────────────────────────────────────────────┐
│              clawfarm-masterpool             │
│                  (Program)                   │
├─────────────────────────────────────────────┤
│  Config PDA        │ rules, ratios, state   │
│  Master Pool Vault │ CLAW tokens (PDA-owned)│
│  Treasury Vault    │ USDC (PDA-owned)       │
│  Pool Authority    │ PDA signer             │
└─────────────────────────────────────────────┘
```

## Key Rules

| Parameter | Value |
|-----------|-------|
| Compute Pool | 50% |
| Outcome Pool | 50% |
| Treasury Tax | 3% of billed usage (USDC) |
| Genesis Supply | 1,000,000,000 CLAW |
| Decimals | 6 |

## Hard Constraints

- Master Pool Vault is **program-owned** (PDA)
- No direct private-key withdrawal path
- Bots can submit settlement but cannot move pool funds
- Mint authority revoked after Genesis mint (permanent)
- Freeze authority revoked after Genesis mint (permanent)
- Upgrade authority under multisig/timelock, then revoked

## Deployment Phases

### Phase A — Infrastructure
1. Deploy program
2. Create Config, Master Pool Vault, Treasury Vault PDAs
3. Publish addresses on clawfarm.network/masterpool

### Phase B — Genesis Mint
1. Mint 1B CLAW to Master Pool Vault
2. Revoke mint authority
3. Revoke freeze authority

### Phase C — Settlement
1. Enable settlement
2. Submit epoch results
3. Distribute compute + outcome rewards

### Phase D — Final Freeze
1. Freeze upgrade authority

## Development

```bash
# Build
anchor build

# Test (localnet)
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

## Instructions

| Instruction | Phase | Description |
|-------------|-------|-------------|
| `initialize_master_pool` | A | Create config with rules |
| `create_master_pool_vault` | A | Create CLAW vault PDA |
| `create_treasury_vault` | A | Create USDC vault PDA |
| `mint_genesis_supply` | B | One-time 1B CLAW mint |
| `revoke_mint_authority` | B | Permanent, irreversible |
| `revoke_freeze_authority` | B | Permanent, irreversible |
| `submit_epoch_settlement` | C | Bot submits settlement |
| `distribute_compute_rewards` | C | 50% by billed usage |
| `distribute_outcome_rewards` | C | 50% by settled tasks |
| `finalize_epoch` | C | Advance epoch counter |
| `enable_settlement` | C | Admin enables settlement |
| `finalize_upgrade_freeze` | D | Permanent, irreversible |

## License

MIT
