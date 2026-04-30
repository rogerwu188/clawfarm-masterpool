# Phase 1 Post-Smoke Validation Checklist

Date: 2026-04-21
Status: Ready to execute after compact receipt smoketest passed on devnet

## 1. 目的

本文档用于承接已经通过的 Phase 1 devnet smoke test，继续完成剩余的
完整功能验收。当前目标不是重新部署，而是基于已经跑通的现网状态，
继续完成：

- T+1 `finalize_receipt`
- `CLAW` reward release + claim
- challenge 三条路径
- provider `exit_provider`

当前推荐入口：

- `yarn phase1:post-smoke:devnet`
- `<clawfarm-masterpool-repo>/scripts/phase1/post-smoke-validation.ts`

## 2. 当前基线

截至 2026-04-21，本次 compact receipt smoke 已通过，关键结果如下：

- smoke 报告：`<clawfarm-masterpool-repo>/tmp/phase1-smoketest-report.json`
- smoke 执行时间：
  - start: `2026-04-21T12:53:04.969Z`
  - finish: `2026-04-21T12:53:32.213Z`
- provider signer upsert tx:
  - `XiSLLmYTjHD4AWArVgFePkPWwByNkpmNgFfvNYeBcdDu29JhcFTBNPfSrSZ3VyxmKrKXp1uQ9BSzp76nzhAZ1SG`
- 正向 receipt submit tx:
  - `4aJDCEUkHp4jp6qS8gHuh5uc9XrSCg1hTXrKBa9HerwghGSbTLce6WsJY1ATadEEtwP6oTGFp2r8yiwXc1xxE2Vu`
- smoke receipt 标识：
  - `receiptHashHex = 0x3ba3a582d0f5aebe8a0e25c2e91d5a975526f6ad0a6aaf02cb4e1930fe66b187`
  - `receiptPda = GAHA6Kn1iYFFaYkFAdatnGdit93RVqJCTW9Sdwn7vNUV`
  - `settlementPda = C47J7rfZRsfYS2thTLD5kHTtaxMXmhD29Zr24MFEzqWf`
- 两个负向校验均已通过：
  - provider 注册侧：`InvalidUsdcMint`
  - receipt 提交侧：`InvalidUsdcMint`
- 正向经济分账已通过：
  - treasury vault: `3000000 -> 6000000`
  - provider pending vault: `7000000 -> 14000000`

结论：

- 固定 `CLAW` / 固定 `Test USDC` 绑定已生效
- compact receipt ABI 已在 devnet 生效
- `receipt_hash -> receipt PDA` 查询已生效
- Phase 1 还差 finalize / challenge / exit 这部分完整闭环

## 3. 执行前提

- 默认目标网络：Solana devnet
- 当前 deploy + bootstrap 已完成，不需要重新初始化
- 当前文档基于以下部署记录：
  - `deployments/devnet-phase1.json`
- 后续调用顺序，以 `tests/phase1-integration.ts` 为唯一代码参考
- 当前仓库已提供后续验证脚本，覆盖：
  - smoke receipt 的 `finalize + reward release/claim`
  - challenge `rejected`
  - challenge `accepted`
  - challenge `timeout reject`（支持分两次执行）
- `provider exit` 仍建议按 `tests/phase1-integration.ts` 手工执行

## 4. 推荐执行顺序

- 先做 smoke receipt 的 T+1 finalize
- 再做 reward release + claim
- 再做 challenge rejected / accepted / timeout reject
- 最后做 provider exit

这样做的原因：

- 可以先把已经上链成功的 smoke receipt 闭环掉
- 再验证锁仓释放逻辑
- 再验证 challenge 的三种经济结果
- 最后清 provider 状态，避免中途 exit 影响后续 challenge 验证

脚本状态说明：

- 若 smoke receipt 仍在 challenge window 内，脚本返回 `waiting`
- 若 timeout challenge 还未达到 `challenge_resolution_timeout_seconds`，脚本同样返回 `waiting`
- `waiting` 表示“链上时间窗口未到”，不是实现错误

## 5. Step A - T+1 Finalize Smoke Receipt

### 5.1 目标

对本次 smoke receipt 执行 `finalize_receipt`，确认：

- attestation `Receipt.status` 从 `Submitted(0)` 进入 `Finalized(2)`
- masterpool `ReceiptSettlement.status` 从 `Recorded(0)` 进入 `FinalizedSettled(1)`
- provider-share USDC 从 `providerPendingUsdcVault` 释放到 provider wallet
- user/provider pending `CLAW` 奖励进入 locked 状态

### 5.2 时间窗口

当前部署默认 challenge window 为 `86400` 秒。

本次 smoke receipt 完成时间是：

- UTC: `2026-04-21 12:53:32`
- Asia/Shanghai: `2026-04-21 20:53:32`

因此，如 challenge window 未被修改，最早建议在以下时间之后执行
`finalize_receipt`：

- UTC: `2026-04-22 12:53:33`
- Asia/Shanghai: `2026-04-22 20:53:33`

### 5.3 执行动作

- 使用 smoke receipt：
  - `receiptPda = GAHA6Kn1iYFFaYkFAdatnGdit93RVqJCTW9Sdwn7vNUV`
  - `settlementPda = C47J7rfZRsfYS2thTLD5kHTtaxMXmhD29Zr24MFEzqWf`
- 运行：
  - `yarn phase1:post-smoke:devnet --deployment deployments/devnet-phase1.json --config ./tmp/post-smoke-validation.json --out ./tmp/phase1-post-smoke-report.json`
- 若未到时间，报告中的 `steps.finalize.action = waiting`
- 到点后重跑同一命令，脚本会自动执行 `finalize_receipt`

### 5.4 通过标准

- `Receipt.status == 2`
- `Receipt.finalized_at > 0`
- `ReceiptSettlement.status == 1`
- `providerPendingUsdcVault` 比 finalize 前减少 `7000000`
- provider wallet 的 Test USDC 比 finalize 前增加 `7000000`
- `reward_lock_started_at > 0`

### 5.5 需归档证据

- finalize tx signature
- finalize 前后 provider wallet USDC 余额
- finalize 前后 `providerPendingUsdcVault` 余额
- finalize 后 `Receipt` 和 `ReceiptSettlement` 读取结果

## 6. Step B - Reward Release + Claim

### 6.1 目标

验证锁仓 `CLAW` 的释放与领取逻辑，确认：

- 可按已解锁额度释放
- 可按已释放额度领取
- 超额释放会被拒绝

### 6.2 执行动作

- 在 smoke receipt finalize 完成后，选取：
  - user reward account
  - provider reward account
- 按 `tests/phase1-integration.ts` 的顺序执行：
  - `materialize_reward_release`
  - `claim_released_claw`

建议至少验证两种情况：

- 合法释放：释放一小部分已归属 `CLAW`
- 非法释放：请求释放超过当前可归属额度，预期报
  `RewardReleaseExceedsVested`

### 6.3 通过标准

- 合法 `materialize_reward_release` 成功
- 合法 `claim_released_claw` 成功
- reward account 中：
  - `locked_claw_total` 按预期下降
  - `released_claw_total` / `claimed_claw_total` 按预期变化
- 超额释放失败，错误符合预期

### 6.4 需归档证据

- release tx signature
- claim tx signature
- 超额释放失败日志
- reward account 前后状态截图或 JSON

## 7. Step C - Challenge 三条路径

### 7.1 共同前提

- 每条 challenge 路径都使用一笔新的 receipt
- 每条路径都先正常 `submit_receipt`
- `challenger` 钱包提前准备足够 `CLAW` 用于 challenge bond
- 建议在 `post-smoke-validation.json` 中为三条路径提供固定 `requestNonce`
  以便脚本支持幂等重跑

### 7.2 Path 1 - Rejected

目标：

- challenge 打开后被 resolver 驳回
- receipt 之后仍然可以正常 `finalize_receipt`

执行顺序：

- `submit_receipt`
- `open_challenge`
- `resolve_challenge(..., resolutionCode = 2)`
- `finalize_receipt`

通过标准：

- challenge 状态进入 `Rejected`
- receipt 最终可 finalize
- provider payout 正常完成

需归档：

- submit tx
- open challenge tx
- resolve challenge tx
- finalize tx

### 7.3 Path 2 - Accepted

目标：

- challenge 被接受
- receipt settlement 回滚
- payer 仅拿回 provider-share USDC
- treasury share 保留
- provider 发生 `CLAW` slash
- challenger 获得奖励

执行顺序：

- `submit_receipt`
- `open_challenge`
- `resolve_challenge(..., resolutionCode = 1)`

脚本额外校验：

- 自动补做一次 `finalize_receipt`
- 预期命中 `ReceiptNotFinalizable`

通过标准：

- settlement 进入 challenged reverted 路径
- provider-share USDC 退回 payer
- treasury share 不退
- provider `claw_net_position` 发生预期变化
- challenger 获得奖励
- 该笔 receipt 不再允许正常 finalize

需归档：

- submit tx
- open challenge tx
- resolve challenge tx
- payer / provider / challenger / treasury 的相关余额变化

### 7.4 Path 3 - Timeout Reject

目标：

- challenge 打开但不人工 resolve
- 等待超时后由 authority 执行 `timeout_reject_challenge`
- receipt 回到可 finalize 状态并完成 payout

执行顺序：

- `submit_receipt`
- `open_challenge`
- 等待 `challenge_resolution_timeout_seconds`
- `timeout_reject_challenge`
- `finalize_receipt`

脚本执行特性：

- 第一次运行通常会停在 `waiting`
- 到达 timeout 后，重跑同一命令即可继续完成

通过标准：

- challenge 最终进入 rejected
- receipt 最终可 finalize
- provider payout 正常完成

需归档：

- submit tx
- open challenge tx
- timeout reject tx
- finalize tx

## 8. Step D - Provider Exit

### 8.1 目标

验证 provider 在所有义务清空后可以退出，且退出后不能再接收新 receipt。

### 8.2 前提条件

执行 `exit_provider` 前必须满足：

- `pendingProviderUsdc = 0`
- `unsettledReceiptCount = 0`
- `unresolvedChallengeCount = 0`

### 8.3 执行动作

- 读取 provider account，确认三个计数/余额归零
- 执行 `exit_provider`
- 再尝试向该 provider 提交一笔新 receipt

### 8.4 通过标准

- `ProviderAccount.status == Exited`
- provider 成功取回 staked USDC
- 对已退出 provider 提交新 receipt 失败，错误为 `ProviderNotActive`

### 8.5 需归档证据

- exit tx signature
- provider stake 返还前后余额
- exit 后再次 submit 的失败日志

## 9. 最终验收结论标准

当以下条件全部满足时，可认定本轮 Phase 1 devnet 验收完成：

- compact receipt smoke 已通过
- 至少 1 笔 receipt 完成 T+1 finalize
- 至少 1 次 reward release + claim 成功
- challenge rejected / accepted / timeout reject 各至少 1 次通过
- provider exit 成功
- exit 后新 receipt 被拒绝

## 10. 建议归档清单

建议最终统一归档到一次执行记录中：

- deployment record:
  - `deployments/devnet-phase1.json`
- smoke report:
  - `tmp/phase1-smoketest-report.json`
- smoke 的关键 tx：
  - provider signer upsert
  - smoke receipt submit
- T+1 finalize tx
- reward release + claim tx
- challenge 三条路径的 open / resolve / timeout / finalize tx
- provider exit tx
- 关键账户前后状态：
  - provider account
  - receipt
  - receipt settlement
  - reward account
  - treasury / pending vault

## 11. 备注

- 现在可以认为“部署 + 固定代币绑定 + compact receipt 上报”已经打通
- 但在执行完本文档第 5 到第 8 节前，还不应宣称
  “Phase 1 全量验收完成”
