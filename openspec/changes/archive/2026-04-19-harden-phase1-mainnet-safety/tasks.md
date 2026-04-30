## 1. Bootstrap authorization and state model

- [x] 1.1 Add upgrade-authority-backed bootstrap checks to `clawfarm-masterpool::initialize_masterpool` and `clawfarm-attestation::initialize_config`
- [x] 1.2 Extend account/state definitions for provisional reward balances, bounded challenge-resolution timeout, and wider signed provider reward accounting
- [x] 1.3 Replace overflow-prone split validation and signed accounting helpers with checked arithmetic and explicit range validation

## 2. Provider and receipt settlement hardening

- [x] 2.1 Enforce `ProviderStatus::Active` in receipt recording and preserve terminal `Exited` behavior in the existing provider lifecycle
- [x] 2.2 Rework `record_mining_from_receipt` to write provisional reward balances and snapshot any data needed to unwind provisional provider reward effects later
- [x] 2.3 Update finalized receipt settlement so valid receipts promote provisional rewards into locked balances while releasing provider-share USDC
- [x] 2.4 Ensure reward release and reward claim flows operate only on finalized locked balances and cannot consume provisional reward state

## 3. Challenge reversal and timeout fallback

- [x] 3.1 Update successful challenge settlement to unwind provisional receipt rewards before applying slash, challenger reward, burn, and payer refund effects
- [x] 3.2 Add the attestation timeout-reject instruction and config wiring for stale open challenges
- [x] 3.3 Integrate timeout rejection with existing rejected-challenge economics so stale challenges burn the bond and leave the receipt eligible for normal finalization
- [x] 3.4 Add invalid-transition guards that prevent duplicate timeout resolution, duplicate reversal, or post-finalization rollback

## 4. Verification and rollout readiness

- [x] 4.1 Add unit and integration tests for unauthorized initialization, exited-provider rejection, provisional-to-locked promotion, and successful-challenge reward reversal
- [x] 4.2 Add tests for timeout fallback, stale-challenge recovery, and rejection of overflowed or out-of-range governance parameters
- [x] 4.3 Update program READMEs and operational notes to document bootstrap authorization, provisional reward semantics, and the authority-driven timeout fallback
