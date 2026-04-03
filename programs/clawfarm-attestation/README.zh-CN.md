# clawfarm-attestation

`clawfarm-attestation` 是 Clawfarm Phase 1 的独立 Solana Receipt Attestation 合约。

英文版：

- [README.md](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/README.md)

它的职责包括：

- 维护 Provider Signer 注册表
- 校验 Provider 对 canonical receipt digest 的签名
- 通过 `request_nonce` 防止重放
- 管理治理驱动的 challenge 生命周期
- 在终态后关闭 receipt 和 challenge 账户以回收 rent

本文档描述的是当前仓库中的实际 on-chain 实现。

## 高层模型

Phase 1 使用最小化的链上 receipt 锚点。

完整 receipt 预期存放在链下，例如由 Clawfarm 托管的 S3。链上只保留：

- `receipt_hash`
- `signer`
- 生命周期时间戳
- receipt 状态

当前信任边界是：

1. 链下先把完整 receipt canonicalize
2. 程序在链上重建同一份 canonical payload
3. 程序校验 `sha256(canonical_payload) == receipt_hash`
4. 程序校验前一条 `ed25519` 验签指令
5. 程序只保存以 `request_nonce` 为键的最小 receipt 锚点

## 程序状态

状态定义见 [accounts.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/state/accounts.rs)。

### `Config`

PDA Seed：

- `["config"]`

字段：

- `authority`
- `pause_authority`
- `challenge_resolver`
- `challenge_window_seconds`
- `is_paused`

用途：

- 全局治理和时间窗口配置

### `ProviderSigner`

PDA Seed：

- `["provider_signer", sha256(provider_code), signer_pubkey]`

字段：

- `provider_code`
- `signer`
- `key_id`
- `attester_type_mask`
- `status`
- `valid_from`
- `valid_until`
- `metadata_hash`
- `created_at`
- `updated_at`

用途：

- 链上 Provider / Gateway Signer 注册表

### `Receipt`

PDA Seed：

- `["receipt", sha256(request_nonce)]`

字段：

- `receipt_hash`
- `signer`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `status`

用途：

- 以 `request_nonce` 为键的重放锁
- 链下完整 receipt 的链上锚点
- challenge 和 close 的状态机

### `Challenge`

PDA Seed：

- `["challenge", receipt.key(), challenge_type, challenger.key()]`

字段：

- `receipt`
- `challenger`
- `challenge_type`
- `evidence_hash`
- `opened_at`
- `resolved_at`
- `status`
- `resolution_code`

用途：

- 针对某个 receipt 的单条争议实例

## 枚举值

定义见 [types.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/state/types.rs)。

### `ProofMode`

- `0 = SigLog`
- `1 = SigLogZkReserved`

Phase 1 只接受 `SigLog`。

### `AttesterType`

- `0 = Provider`
- `1 = Gateway`
- `2 = Hybrid`

### `UsageBasis`

- `0 = ProviderReported`
- `1 = ServerEstimatedReserved`
- `2 = HybridReserved`
- `3 = TokenizerVerifiedReserved`

Phase 1 只接受 `ProviderReported`。

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
- `3 = Expired`

说明：

- `Expired` 目前只预留在枚举中，当前实现没有任何指令会写入该状态

### `ChallengeType`

- `0 = InvalidSignature`
- `1 = SignerRegistryMismatch`
- `2 = ReplayNonce`
- `3 = InvalidLogInclusion`
- `4 = PayloadMismatch`

### `ResolutionCode`

- `0 = None`
- `1 = Accepted`
- `2 = Rejected`
- `3 = ReceiptInvalidated`
- `4 = SignerRevoked`

## Canonical Receipt 约定

Canonicalization 逻辑见 [utils.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/utils.rs)。

规则：

- 程序接收结构化的 `SubmitReceiptArgs`
- 程序在链上重建确定性的 CBOR payload
- 可选字段缺失时不编码
- `receipt_hash = sha256(canonical_cbor_bytes)`
- 交易中前一条指令必须是针对该 32 字节 digest 的匹配 `ed25519` 验签指令

以下字段不会进入签名 payload：

- `signer`
- `receipt_hash`

也就是说，传输层的 receipt 元数据完全留在链下；链上只绑定 canonical digest 和最小生命周期状态。

## 推荐的 Clawfarm S3 流程

本仓库不包含链下存储服务实现，但当前合约就是为下面这套运维流程设计的。

### 参与方

- `Provider`：返回原始 usage receipt
- `Clawfarm service`：规范化 receipt、上传 S3、并发起上链交易
- `Clawfarm website`：提供用户、challenger、客服的查询入口
- `clawfarm-attestation`：校验 digest 并维护链上生命周期

### 推荐流程

1. Provider 向 Clawfarm 返回完整 receipt payload。
2. Clawfarm 校验 payload 结构，并规范化成 Phase 1 canonical schema。
3. Clawfarm 计算 canonical CBOR 和 `receipt_hash`。
4. Clawfarm 将完整 canonical receipt 上传到 S3。
5. Clawfarm 在自己的索引里保存元数据，例如：
   - `receipt_hash`
   - `request_nonce`
   - `provider`
   - `proof_id`
   - S3 object key
   - submission status
6. Clawfarm 组装 `ed25519` 验签指令并发送 `submit_receipt`。
7. 官网支持按 `receipt_hash`、`request_nonce` 或内部 id 查询。
8. 发生 challenge 时，Clawfarm 或 challenger 从 S3 取回完整 receipt，构造证据包，上链只提交证据哈希。
9. Receipt 进入终态后，Clawfarm 调用 `close_challenge` 和 `close_receipt` 回收 rent。

### 建议的 S3 对象路径

- `receipts/{provider}/{yyyy}/{mm}/{receipt_hash}.json`
- 或 `receipts/{receipt_hash}.cbor`

### 建议的链下索引字段

- `receipt_hash`
- `request_nonce`
- `provider`
- `proof_id`
- `signer`
- `submitted_at`
- `challenge_deadline`
- `finalized_at`
- `receipt_status`
- `challenge_status`
- `s3_bucket`
- `s3_key`
- `content_type`
- `schema_version`

### 运维说明

- 真正的信任锚点是链上的 `receipt_hash`，不是 S3 URL
- S3 对象上传后应视为不可变
- 最好启用 bucket versioning，并禁止覆盖同名对象
- 官网应查询 Clawfarm 的索引层，而不是只靠 S3 列表推导状态
- close 流程只能在链上确认终态后执行

## Resolver 机器人流程

这里的 `challenge_resolver` 更适合设计成 Clawfarm 的自动化机器人，而不是人工手动操作的钱包。

推荐工作循环：

1. 监听新的 `ChallengeOpened` 事件，或定时扫描状态仍为 `Open` 的 challenge PDA
2. 通过 Clawfarm 索引读取关联 receipt，并从链下存储拉取完整 canonical receipt 与 challenge 证据
3. 在链下重建 dispute package，并按 `challenge_type` 执行 Clawfarm 自己的校验逻辑
4. 根据校验结果归约成单个 `resolution_code`：
   - challenge 不成立时用 `Rejected`
   - receipt 无效时用 `Accepted` 或 `ReceiptInvalidated`
   - signer 需要连带惩罚和撤销时用 `SignerRevoked`
5. 由机器人控制的 `challenge_resolver` 权限地址发起 `resolve_challenge`
6. receipt 和 challenge 进入终态后，再执行 `close_challenge` 与 `close_receipt` 回收 rent

运维建议：

- 机器人在链上尽量保持无状态，持久事实来源应仍然是 Clawfarm 索引和链上的 receipt/challenge PDA
- 链下校验逻辑要尽量确定性、可重放，保证后续审计时能解释某个 `resolution_code` 为什么成立
- 调度要做成幂等；如果 RPC 提交失败或取证过程中断，机器人应能安全重试
- 机器人日志里最好记录证据对象的版本号或内容哈希，方便追溯当时裁决使用的具体材料

## Rent 估算

当前实现通过最小化 `ReceiptLite` 并在终态后及时 close，来降低长期成本。

### 当前账户大小

- `Receipt` 分配大小：`97 bytes`
- `Challenge` 分配大小：`123 bytes`

### Rent 公式

按当前 Solana 的 rent-exempt 公式：

```text
minimum_balance = (account_data_len + 128) * 6,960 lamports
```

可得到：

- 每个 `Receipt`：
  - `(97 + 128) * 6,960 = 1,566,000 lamports = 0.001566 SOL`
- 每个 `Challenge`：
  - `(123 + 128) * 6,960 = 1,746,960 lamports = 0.00174696 SOL`

重要说明：

- 这部分是 rent collateral，不是永久烧掉的 gas
- 成功执行 `close_receipt` 或 `close_challenge` 后，对应 lamports 会退回

### 峰值 collateral 公式

如果 receipt 在 challenge window 结束后就 close，那么稳态峰值占用大约是：

```text
receipt_peak_sol
  = daily_call_count * challenge_window_days * 0.001566
```

如果每个 receipt 同时都存在一个 live challenge，那么保守上限是：

```text
receipt_plus_challenge_peak_sol
  = daily_call_count * challenge_window_days * 0.00331296
```

### 仅 Receipt 的峰值占用

假设每次调用都创建一个 `Receipt`，并在 challenge window 结束后 close：

| 每日调用量 | 1 天窗口 | 3 天窗口 | 7 天窗口 |
|---|---:|---:|---:|
| 1,000 | 1.566 SOL | 4.698 SOL | 10.962 SOL |
| 10,000 | 15.66 SOL | 46.98 SOL | 109.62 SOL |
| 100,000 | 156.6 SOL | 469.8 SOL | 1096.2 SOL |

### 保守上限：每个 Receipt 都同时存在一个 Challenge

假设每个 receipt 都伴随一个 live `Challenge`：

| 每日调用量 | 1 天窗口 | 3 天窗口 | 7 天窗口 |
|---|---:|---:|---:|
| 1,000 | 3.31296 SOL | 9.93888 SOL | 23.19072 SOL |
| 10,000 | 33.1296 SOL | 99.3888 SOL | 231.9072 SOL |
| 100,000 | 331.296 SOL | 993.888 SOL | 2319.072 SOL |

### 如何理解这些数字

- 如果 challenge 率很低，真实占用会更接近“仅 Receipt”这张表
- 真正的优化重点不是基础交易费，而是降低并回收 rent collateral
- 缩短 `challenge_window_seconds` 会直接降低 rent 峰值占用

## 接口说明

入口定义见 [lib.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/lib.rs)。

## 1. `initialize_config`

实现位置：

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L11)

签名：

```rust
pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    authority: Pubkey,
    pause_authority: Pubkey,
    challenge_resolver: Pubkey,
    challenge_window_seconds: i64,
) -> Result<()>
```

账户：

- `payer`：签名者，支付 `Config` 的 rent
- `config`：通过 `["config"]` 初始化的 config PDA
- `system_program`

入参说明：

- `authority`：主治理权限地址
- `pause_authority`：可切换 pause 的权限地址
- `challenge_resolver`：可裁决 challenge 的权限地址，通常由 Clawfarm 的自动化 resolver 机器人持有
- `challenge_window_seconds`：receipt 可被 challenge 的窗口，必须 `> 0`

功能流程：

1. 校验 challenge window 大于 0
2. 初始化 config PDA
3. 写入治理地址和时间窗口
4. 设置 `is_paused = false`
5. 发出 `ConfigInitialized`

结果：

- 唯一的 `Config` 账户被创建，程序可开始管理 signer 和 receipt

## 2. `upsert_provider_signer`

实现位置：

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L40)

签名：

```rust
pub fn upsert_provider_signer(
    ctx: Context<UpsertProviderSigner>,
    provider_code: String,
    signer: Pubkey,
    key_id: String,
    attester_type_mask: u8,
    valid_from: i64,
    valid_until: i64,
    metadata_hash: [u8; 32],
) -> Result<()>
```

账户：

- `authority`：签名者，必须等于 `config.authority`
- `config`：config PDA
- `provider_signer`：由 `provider_code + signer` 导出的 PDA
- `system_program`

入参说明：

- `provider_code`：provider 标识
- `signer`：provider 或 gateway 的签名公钥
- `key_id`：链下 key 标识
- `attester_type_mask`：允许的 attester type 位图
- `valid_from`：生效时间
- `valid_until`：失效时间，`0` 表示不设上限
- `metadata_hash`：链下 signer metadata 的哈希

功能流程：

1. 校验 `provider_code`
2. 校验 `key_id`
3. 要求 `attester_type_mask != 0`
4. 要求 `valid_until == 0 || valid_until >= valid_from`
5. 使用 `init_if_needed` 创建或复用 signer PDA
6. 如果是旧账户则保留 `created_at`
7. 覆盖写入 signer registry 字段
8. 强制将状态设为 `Active`
9. 更新时间戳
10. 发出 `ProviderSignerUpserted`

结果：

- provider signer 记录存在，并可用于 `submit_receipt`

## 3. `set_pause`

实现位置：

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L85)

签名：

```rust
pub fn set_pause(ctx: Context<SetPause>, is_paused: bool) -> Result<()>
```

账户：

- `pause_authority`：签名者，必须等于 `config.pause_authority`
- `config`：config PDA

入参说明：

- `is_paused`：目标 pause 状态

功能流程：

1. 通过 Anchor 约束校验 pause authority
2. 写入 `config.is_paused`
3. 发出 `PauseUpdated`

结果：

- 后续 `submit_receipt` 是否允许执行由该标志决定

说明：

- 当前实现只有 `submit_receipt` 显式检查 `is_paused`

## 4. `revoke_provider_signer`

实现位置：

- [admin.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/admin.rs#L93)

签名：

```rust
pub fn revoke_provider_signer(
    ctx: Context<RevokeProviderSigner>,
    provider_code: String,
    signer: Pubkey,
) -> Result<()>
```

账户：

- `authority`：签名者，必须等于 `config.authority`
- `config`：config PDA
- `provider_signer`：目标 signer PDA

入参说明：

- `provider_code`：provider 标识
- `signer`：待撤销的 signer 公钥

功能流程：

1. 校验 `provider_code`
2. 检查加载到的 signer 账户与 `provider_code` 一致
3. 检查加载到的 signer 账户与 `signer` 一致
4. 将状态改为 `Revoked`
5. 更新 `updated_at`
6. 发出 `ProviderSignerRevoked`

结果：

- 该 signer 不再能用于后续 receipt 提交

## 5. `submit_receipt`

实现位置：

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs)

签名：

```rust
pub fn submit_receipt(ctx: Context<SubmitReceipt>, args: SubmitReceiptArgs) -> Result<()>
```

账户：

- `payer`：签名者，支付 `Receipt` 的 rent
- `config`：只读 config PDA
- `provider_signer`：signer registry PDA
- `receipt`：由 `request_nonce` 导出的 receipt PDA
- `instructions_sysvar`：Solana instruction sysvar，用于 `ed25519` introspection
- `system_program`

指令参数：

```rust
pub struct SubmitReceiptArgs {
    pub version: u8,
    pub proof_mode: u8,
    pub proof_id: String,
    pub request_nonce: String,
    pub provider: String,
    pub attester_type: u8,
    pub model: String,
    pub usage_basis: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub charge_atomic: u64,
    pub charge_mint: Pubkey,
    pub provider_request_id: Option<String>,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub receipt_hash: [u8; 32],
    pub signer: Pubkey,
}
```

参数含义：

- `version`：必须是 `1`
- `proof_mode`：必须是 `SigLog`
- `proof_id`：provider 侧 proof 标识
- `request_nonce`：防重放唯一业务 nonce
- `provider`：provider code
- `attester_type`：provider / gateway / hybrid
- `model`：链下模型标识
- `usage_basis`：必须是 `ProviderReported`
- `prompt_tokens`：输入 token 数
- `completion_tokens`：输出 token 数
- `total_tokens`：必须等于前两者之和
- `charge_atomic`：最小单位计价金额
- `charge_mint`：收费 mint
- `provider_request_id`：可选 provider 原始请求 id
- `issued_at`：可选签发时间
- `expires_at`：可选过期时间
- `http_status`：可选 HTTP 状态码
- `latency_ms`：可选请求耗时
- `receipt_hash`：canonical receipt digest
- `signer`：签名公钥；交易前一条 `ed25519` 指令必须验证该公钥对 `receipt_hash` 的签名

功能流程：

1. 校验所有结构化字段
2. 检查程序未 pause
3. 加载并校验 provider signer registry
4. 在链上重建 canonical CBOR
5. 计算哈希并要求其等于 `receipt_hash`
6. 检查前一条交易指令是否为匹配的 `ed25519` 验签，且验证的是 `signer` 对 `receipt_hash` 的签名
7. 创建 `Receipt` PDA
8. 只写入 `receipt_hash`、`signer`、时间戳和状态
9. 发出 `ReceiptSubmitted`

结果：

- 以该 `request_nonce` 为键的唯一 `Receipt` 账户创建成功
- 完整 receipt 仍在链下，但被链上的 `receipt_hash` 锚定

## 6. `open_challenge`

实现位置：

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs)

签名：

```rust
pub fn open_challenge(
    ctx: Context<OpenChallenge>,
    challenge_type: u8,
    evidence_hash: [u8; 32],
) -> Result<()>
```

账户：

- `challenger`：签名者，支付 `Challenge` 的 rent
- `receipt`：目标 receipt 账户
- `challenge`：由 `(receipt, challenge_type, challenger)` 导出的 challenge PDA
- `system_program`

入参说明：

- `challenge_type`：争议类型
- `evidence_hash`：链下证据哈希

功能流程：

1. 校验 `challenge_type`
2. 检查 receipt 当前仍是 `Submitted`
3. 检查当前时间仍在 challenge window 内
4. 创建 `Challenge` PDA
5. 写入 challenger、evidence hash、时间戳和状态
6. 将 receipt 状态设为 `Challenged`
7. 发出 `ChallengeOpened`

结果：

- 一条 challenge 被创建
- receipt 进入 `Challenged` 状态

## 7. `resolve_challenge`

实现位置：

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs)

签名：

```rust
pub fn resolve_challenge(ctx: Context<ResolveChallenge>, resolution_code: u8) -> Result<()>
```

账户：

- `challenge_resolver`：签名者，必须等于 `config.challenge_resolver`
- `config`：config PDA
- `receipt`：关联 receipt 账户
- `challenge`：目标 challenge 账户；必须指向该 `receipt`

入参说明：

- `resolution_code`：最终裁决结果

功能流程：

1. 校验 `resolution_code`，且不能是 `None`
2. 检查 challenge 当前是 `Open`
3. 检查 `challenge.receipt == receipt.key()`
4. 写入 `resolution_code` 和 `resolved_at`
5. 将 receipt 直接推进到终态：
   - `Accepted` 或 `ReceiptInvalidated` -> `Rejected`
   - `SignerRevoked` -> `Slashed`
   - `Rejected` -> `Finalized`
6. 设置 `receipt.finalized_at = now`
7. 发出 `ChallengeResolved`

结果：

- receipt 离开 active dispute 状态，后续可进入 close 流程

## 8. `finalize_receipt`

实现位置：

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs)

签名：

```rust
pub fn finalize_receipt(ctx: Context<FinalizeReceipt>) -> Result<()>
```

账户：

- `receipt`：目标 receipt 账户

功能流程：

1. 检查 receipt 仍然是 `Submitted`
2. 检查 `now > challenge_deadline`
3. 将 receipt 状态设为 `Finalized`
4. 设置 `finalized_at = now`
5. 发出 `ReceiptFinalized`

结果：

- 未被 challenge 的 receipt 进入终态，可后续 close

## 9. `close_challenge`

实现位置：

- [challenge.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/challenge.rs)

签名：

```rust
pub fn close_challenge(ctx: Context<CloseChallenge>) -> Result<()>
```

账户：

- `recipient`：签名者，接收回收的 lamports
- `challenge`：终态 challenge 账户

功能流程：

1. 检查 challenge 状态已是终态：
   - `Accepted`
   - `Rejected`
   - `Expired`
2. 发出 `ChallengeClosed`
3. 通过 Anchor 的 `close = recipient` 关闭 challenge 账户

结果：

- challenge 的 rent 退回给 `recipient`

## 10. `close_receipt`

实现位置：

- [receipt.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/instructions/receipt.rs)

签名：

```rust
pub fn close_receipt(ctx: Context<CloseReceipt>) -> Result<()>
```

账户：

- `recipient`：签名者，接收回收的 lamports
- `receipt`：终态 receipt 账户

功能流程：

1. 检查 receipt 状态已是终态：
   - `Finalized`
   - `Rejected`
   - `Slashed`
2. 发出 `ReceiptClosed`
3. 通过 Anchor 的 `close = recipient` 关闭 receipt 账户

结果：

- receipt 的 rent 退回给 `recipient`

## 生命周期总结

Receipt 状态机：

```text
Submitted
  -> Challenged
  -> Finalized

Challenged
  -> Finalized
  -> Rejected
  -> Slashed
```

可关闭的 receipt 状态：

- `Finalized`
- `Rejected`
- `Slashed`

Challenge 状态机：

```text
Open
  -> Accepted
  -> Rejected
```

可关闭的 challenge 状态：

- `Accepted`
- `Rejected`
- `Expired`

## Events

事件定义见 [events.rs](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/programs/clawfarm-attestation/src/events.rs)。

当前事件：

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

## 测试覆盖

当前集成测试位于
[tests/clawfarm-attestation.ts](/Users/lijing/Code/Cobra/Solana/clawfarm-masterpool/tests/clawfarm-attestation.ts)，覆盖了：

- config 初始化
- signer upsert
- 缺失 `ed25519` 预指令时的拒绝路径
- 正常 receipt 提交
- 未被 challenge 的 receipt finalization 和 close
- 被 challenge 的 receipt resolution 和 close
