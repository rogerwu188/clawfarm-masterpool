# Phase 1 Post-Smoke Waiting And Challenges Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the devnet post-smoke script so pre-finalize windows return a non-failing waiting result, and add resumable challenge-path automation for rejected, accepted, and timeout-reject validation.

**Architecture:** Keep the existing `scripts/phase1/post-smoke-validation.ts` entrypoint and expand it with pure helpers for status/config normalization plus inline orchestration for challenge cases. Preserve current smoke-receipt finalize/reward behavior, but allow the script to degrade to `waiting` when the chain state is not yet actionable and to resume timeout challenges from deterministic request nonces.

**Tech Stack:** TypeScript, Anchor, Solana web3.js, SPL Token, ts-mocha

---

### Task 1: Add failing tests for waiting status and challenge config normalization

**Files:**
- Modify: `tests/phase1-post-smoke-validation-script.ts`
- Test: `tests/phase1-post-smoke-validation-script.ts`

- [ ] **Step 1: Add parser/config/status tests for the new waiting + challenge helpers**
- [ ] **Step 2: Run `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-post-smoke-validation-script.ts` and confirm the new assertions fail**
- [ ] **Step 3: Implement the minimal helper exports in `scripts/phase1/post-smoke-validation.ts`**
- [ ] **Step 4: Re-run the same test file and confirm it passes**

### Task 2: Implement waiting semantics for the existing smoke finalize flow

**Files:**
- Modify: `scripts/phase1/post-smoke-validation.ts`
- Test: `tests/phase1-post-smoke-validation-script.ts`

- [ ] **Step 1: Change report status handling to allow `waiting` without throwing**
- [ ] **Step 2: Record a structured finalize waiting section instead of raising an exception when challenge window is still open**
- [ ] **Step 3: Keep reward release logic gated behind finalized settlement state**
- [ ] **Step 4: Re-run the post-smoke unit tests**

### Task 3: Add resumable challenge-path execution to the post-smoke script

**Files:**
- Modify: `scripts/phase1/post-smoke-validation.ts`
- Modify: `scripts/phase1/post-smoke-validation.example.json`
- Test: `tests/phase1-post-smoke-validation-script.ts`

- [ ] **Step 1: Add normalized optional challenge-case config with deterministic request nonces and shared receipt defaults**
- [ ] **Step 2: Reuse compact receipt helpers to submit challenge receipts and derive challenge/bond PDAs**
- [ ] **Step 3: Implement rejected and accepted challenge execution in one run**
- [ ] **Step 4: Implement timeout-reject execution as a resumable flow that returns `waiting` until timeout elapses, then completes timeout reject + finalize**
- [ ] **Step 5: Re-run the post-smoke unit tests**

### Task 4: Update operator-facing examples and run regression checks

**Files:**
- Modify: `scripts/phase1/post-smoke-validation.example.json`
- Modify: `docs/phase1-post-smoke-validation-checklist.md`
- Modify: `package.json` (only if script surface changes)
- Test: `tests/phase1-post-smoke-validation-script.ts`
- Test: `tests/phase1-devnet-smoketest-script.ts`

- [ ] **Step 1: Refresh the example config to show optional challenge cases and required keypairs**
- [ ] **Step 2: Update the checklist doc to explain `waiting` semantics and timeout-challenge reruns**
- [ ] **Step 3: Run `npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-post-smoke-validation-script.ts tests/phase1-devnet-smoketest-script.ts`**
- [ ] **Step 4: Sanity-check the CLI surface with `yarn phase1:post-smoke:devnet --help`**
