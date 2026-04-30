# clawfarm-attestation

`clawfarm-attestation` 是 Clawfarm Phase 1 的 Solana receipt 生命周期合约。

英文版：

- [README.md](README.md)

本文档以当前仓库里的链上实现为准。

源码入口：

- [src/lib.rs](src/lib.rs)
- [src/instructions/admin.rs](src/instructions/admin.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [src/events.rs](src/events.rs)
- [../clawfarm-masterpool/README.md](../clawfarm-masterpool/README.md)
- [../../tests/phase1-integration.ts](../../tests/phase1-integration.ts)

## 职责

- 维护 provider signer 注册表
- 校验 provider 对 canonical receipt digest 的签名
- 通过 `request_nonce` 做重放保护
- 维护 receipt 和 challenge 的生命周期状态
- 通过 CPI 把经济动作转发给 `clawfarm-masterpool`
- 在终态后关闭 `Receipt` 和 `Challenge` 账户回收 rent

## 高层模型

Phase 1 采用最小链上 receipt 锚定模型。

完整 receipt 正文保留在链下。链上只保存：

- `receipt_hash`
- `signer`
- `payer_user`
- `provider_wallet`
- 生命周期时间戳
- receipt 状态
- 经济结果是否已经同步到 masterpool

信任边界如下：

1. 完整 receipt 在链下 canonicalize
2. 合约在链上重建同一份 canonical CBOR
3. 合约校验 `sha256(canonical_payload) == receipt_hash`
4. 合约校验前一条指令是匹配的 `ed25519` verify
5. 合约按 `request_nonce` 创建最小化 `Receipt` PDA
6. 合约再 CPI 调用 masterpool 记录或结算经济侧状态

## 当前实现的关键约束

- 只接受 `version == 1` 的 receipt
- Phase 1 只接受 `ProofMode::SigLog`
- Phase 1 只接受 `UsageBasis::ProviderReported`
- 每个 `request_nonce` 只对应一个 `Receipt` PDA
- 每个 `Receipt` 只对应一个 `Challenge` PDA
- 当前没有 `respond_challenge` 指令
- 完整 receipt 正文和 proof URL 都不在链上保存
- challenge bond 和 challenge 结算现在都在 masterpool 里用 `CLAW` 管理，
  不再是本合约里的 lamports
- `close_receipt` 要求 receipt 已处于终态，且
  `receipt.economics_settled == true`
- 如果 challenge 被驳回，receipt 会先变成 `Finalized`，但仍然需要之后再调用
  一次 `finalize_receipt`，把 provider payout 在 masterpool 里结掉
- `initialize_config` 需要当前程序 upgrade authority 通过 `ProgramData`
  账户签名授权
- config 里保存正数的 `challenge_resolution_timeout_seconds`，用于在正常裁决路径失效时由
  `authority` 触发超时兜底
- provider signer 现在会绑定到唯一的 `provider_wallet`
- 提交时，签名 payload 里的 `payer_user` / `provider_wallet` 必须和运行时账户、
  signer 注册表以及转发到 masterpool 的结算身份一致

## 角色分工

- `authority`
  - 提交 receipt
  - finalize 未被 challenge 的 receipt
  - finalize 已被驳回 challenge、但经济结果仍未结算的 receipt
  - 关闭终态 receipt / challenge 账户
- `pause_authority`
  - 切换程序暂停开关
- `challenge_resolver`
  - 对 open challenge 做裁决
- `challenger`
  - 发起 challenge，并通过 masterpool 支付固定 `CLAW` bond

## 程序状态

状态定义见 [src/state/accounts.rs](src/state/accounts.rs)。

### `Config`

PDA seed：

- `["config"]`

字段：

- `authority`
- `pause_authority`
- `challenge_resolver`
- `masterpool_program`
- `challenge_window_seconds`
- `challenge_resolution_timeout_seconds`
- `is_paused`

用途：

- 保存生命周期治理角色、关联的 `clawfarm-masterpool` 程序、challenge window
  以及超时兜底参数

### `ProviderSigner`

PDA seed：

- `["provider_signer", sha256(provider_code), signer_pubkey]`

字段：

- `provider_wallet`
- `attester_type_mask`
- `status`
- `valid_from`
- `valid_until`

用途：

- 以 provider code 和 signer pubkey 为键维护最小 signer 策略，并把 signer
  固定绑定到单个 provider wallet

### `Receipt`

PDA seed：

- `["receipt", sha256(request_nonce)]`

字段：

- `receipt_hash`
- `signer`
- `payer_user`
- `provider_wallet`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `status`
- `economics_settled`

用途：

- 基于 `request_nonce` 做 replay lock
- 作为链下 receipt 正文的最小链上锚点
- 承载 submit、challenge、finalize、close 的状态机

### `Challenge`

PDA seed：

- `["challenge", receipt.key()]`

字段：

- `receipt`
- `challenger`
- `challenge_type`
- `evidence_hash`
- `bond_amount`
- `opened_at`
- `resolved_at`
- `status`
- `resolution_code`

用途：

- 为单个 receipt 提供唯一一个 dispute slot

## 枚举值

定义见 [src/state/types.rs](src/state/types.rs)。

### `ProofMode`

- `0 = SigLog`
- `1 = SigLogZkReserved`

### `AttesterType`

- `0 = Provider`
- `1 = Gateway`
- `2 = Hybrid`

### `UsageBasis`

- `0 = ProviderReported`
- `1 = ServerEstimatedReserved`
- `2 = HybridReserved`
- `3 = TokenizerVerifiedReserved`

### `SignerStatus`

- `0 = Inactive`
- `1 = Active`
- `2 = Revoked`

### `ReceiptStatus`

- `0 = Submitted`
- `1 = Challenged`
- `2 = Finalized`
- `3 = Rejected`
- `4 = Slashed`

### `ChallengeStatus`

- `0 = Open`
- `1 = Accepted`
- `2 = Rejected`

### `ResolutionCode`

- `0 = None`
- `1 = Accepted`
- `2 = Rejected`
- `3 = ReceiptInvalidated`
- `4 = SignerRevoked`

## `SubmitReceiptArgs` 参数合同

`submit_receipt` 接收结构化的 `SubmitReceiptArgs`。

必填字段：

- `version`
- `proof_mode`
- `proof_id`
- `request_nonce`
- `provider`
- `provider_wallet`
- `payer_user`
- `attester_type`
- `model`
- `usage_basis`
- `prompt_tokens`
- `completion_tokens`
- `total_tokens`
- `charge_atomic`
- `charge_mint`
- `receipt_hash`
- `signer`

可选字段：

- `provider_request_id`
- `issued_at`
- `expires_at`
- `http_status`
- `latency_ms`

当前链上会强制校验：

- `version` 必须为 `1`
- `proof_mode` 必须为 `SigLog`
- `usage_basis` 必须为 `ProviderReported`
- `total_tokens` 必须等于 `prompt_tokens + completion_tokens`
- 字符串字段必须满足 Phase 1 的长度和字符限制
- 签名 payload 里的 `payer_user` 必须等于运行时 payer signer
- 签名 payload 里的 `provider_wallet` 必须同时匹配 signer registry 中登记的钱包，
  以及转发给 masterpool 的 provider 身份
- `http_status`、`issued_at`、`expires_at` 必须内部一致
- 交易前一条指令必须是匹配该 32 字节 digest 的 `ed25519` verify

## 接口总览

入口定义见 [src/lib.rs](src/lib.rs)。

### 管理员接口

- `initialize_config(authority, pause_authority, challenge_resolver, masterpool_program, challenge_window_seconds, challenge_resolution_timeout_seconds)`
  - 创建单例 config
  - 要求当前 upgrade authority 完成 bootstrap 授权
  - 绑定治理角色和关联的 masterpool program
  - 同时写入 challenge window 和 stale challenge 超时阈值
- `upsert_provider_signer(provider_code, signer, provider_wallet, attester_type_mask, valid_from, valid_until)`
  - 创建或更新 provider signer 策略记录
- `set_pause(is_paused)`
  - 切换全局暂停开关
- `revoke_provider_signer(provider_code, signer)`
  - 把 signer 状态设为 revoked

### Receipt 生命周期接口

- `submit_receipt(args: SubmitReceiptArgs)`
  - 校验结构化 payload
  - 校验 provider signer 策略和有效期
  - 重建 canonical CBOR 并校验 `receipt_hash`
  - 校验前一条 `ed25519` 指令
  - 校验签名里的 payer / provider 身份与运行时账户、signer registry、masterpool provider identity 一致
  - 创建 `Receipt` PDA
  - CPI 调用 masterpool 的 `record_mining_from_receipt`
- `finalize_receipt()`
  - 在 challenge window 关闭后 finalize `Submitted` receipt
  - 或对已经 `Finalized` 但经济结果尚未结算的 receipt 补做经济结算
  - CPI 调用 masterpool 的 `settle_finalized_receipt`
  - 把 `receipt.economics_settled` 设为 `true`
- `close_receipt()`
  - 只有在 receipt 已终态且经济结果已同步给 masterpool 后才能关闭

### Challenge 生命周期接口

- `open_challenge(challenge_type, evidence_hash)`
  - 校验 challenge type
  - 要求 receipt 仍处于可 challenge 状态
  - 创建 `Challenge` PDA
  - CPI 调用 masterpool 的 `record_challenge_bond`
  - 把 receipt 状态推进到 `Challenged`
- `resolve_challenge(resolution_code)`
  - 要求 challenge 仍是 open
  - 写入终态的 challenge / receipt 状态
  - CPI 调用 masterpool 的 `resolve_challenge_economics`
  - 只有 `Rejected` 路径会保留 `economics_settled = false`，
    因为 provider payout 还需要后续 finalize
- `timeout_reject_challenge()`
  - 要求 challenge 仍是 open，且已超过超时阈值
  - 要求调用者是 `authority`，不是 `challenge_resolver`
  - 把 challenge 强制标记为 `Rejected`
  - CPI 调用 masterpool 复用现有 rejected-challenge 的 burn 路径
  - 让 receipt 重新回到可后续 `finalize_receipt` 的状态
- `close_challenge()`
  - 在 challenge 到达 `Accepted` 或 `Rejected` 后关闭

## 事件面

事件定义见 [src/events.rs](src/events.rs)。

- `ConfigInitialized`
- `ProviderSignerUpserted`
- `ProviderSignerRevoked`
- `PauseUpdated`
- `ReceiptSubmitted`
- `ReceiptFinalized`
- `ReceiptClosed`
- `ChallengeOpened`
- `ChallengeResolved`
- `ChallengeClosed`

## 生命周期流程

1. 管理员初始化 config，并 upsert 一个或多个 provider signer。
2. `authority` 携带匹配的前置 `ed25519` verify 指令提交 receipt。
3. Attestation 校验该 receipt 签名已经绑定唯一的 `payer_user` 和
   `provider_wallet`，创建 `Receipt` PDA，并 CPI 调用 masterpool 记录 receipt
   经济快照。
4. 在 challenge window 内，challenger 最多只能发起一个 challenge。
5. `challenge_resolver` 对 challenge 作出裁决：
   - `Rejected`：receipt 变成 `Finalized`，challenger bond 在 masterpool 里被销毁，
     但经济结果还需要之后再 finalize
   - `Accepted` 或 `ReceiptInvalidated`：receipt 变成 `Rejected`，
     masterpool 回滚经济结果
   - `SignerRevoked`：receipt 变成 `Slashed`，masterpool 回滚经济结果
6. 如果正常裁决路径没有在
   `opened_at + challenge_resolution_timeout_seconds` 前执行，`authority` 可以调用
   `timeout_reject_challenge`：
   - challenge 会被强制改成 `Rejected`
   - masterpool 通过现有 rejected 路径销毁 challenger bond
   - receipt 重新具备后续 `finalize_receipt` 资格
7. `authority` finalize 未被 challenge 的 receipt，或者 finalize 已被驳回 challenge、
   但尚未结清 provider payout 的 receipt。
8. 当 receipt 到达终态且经济结果已结算后，`authority` 关闭 challenge / receipt
   账户回收 rent。

## 运维说明

- 单例 config 必须由当前升级权限完成 bootstrap，这样公开网络部署时首个调用者无法抢占治理权。
- timeout fallback 刻意保持保守：只有 `authority` 可以触发，且只会把 challenge 拉回现有 `Rejected` 路径，不会引入 permissionless arbitration。
- `Challenged` receipt 在正常 resolver 或 timeout fallback 把它恢复到 rejected/finalized 路径之前，不能直接 finalize。

## 已验证行为

当前端到端集成测试
[../../tests/phase1-integration.ts](../../tests/phase1-integration.ts) 覆盖：

- 未授权 bootstrap 初始化会被拒绝
- provider signer 初始化
- receipt 提交以及 masterpool CPI 记账
- 伪造 payer/provider 结算身份会被拒绝
- 未授权直接调用 masterpool receipt 接口失败
- challenge 被驳回后的 burn 路径以及后续 finalize
- challenge 被接受后的退款与 slash 路径
- stale challenge 的 authority-driven timeout rejection
- 重复 receipt 防重
- `economics_settled` 关闭保护

## 开发

```bash
anchor build
yarn test
```
