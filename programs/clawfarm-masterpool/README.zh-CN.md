# clawfarm-masterpool

`clawfarm-masterpool` 是 Clawfarm Phase 1 的 Solana 经济结算合约。

英文版：

- [README.md](README.md)

本文档以当前仓库里的链上实现为准。

源码入口：

- [src/lib.rs](src/lib.rs)
- [src/instructions/config.rs](src/instructions/config.rs)
- [src/instructions/provider.rs](src/instructions/provider.rs)
- [src/instructions/reward.rs](src/instructions/reward.rs)
- [src/instructions/receipt.rs](src/instructions/receipt.rs)
- [src/instructions/challenge.rs](src/instructions/challenge.rs)
- [src/state/accounts.rs](src/state/accounts.rs)
- [src/state/types.rs](src/state/types.rs)
- [../../docs/phase1-core-economics.md](../../docs/phase1-core-economics.md)
- [../../tests/phase1-integration.ts](../../tests/phase1-integration.ts)

## 职责

- 持有并管理五个协议金库：reward `CLAW`、challenge-bond `CLAW`、
  treasury `USDC`、provider-stake `USDC`、provider-pending `USDC`
- 让 provider 通过固定 USDC 质押完成注册，并在所有义务清空后允许退出
- 接收 attestation 的 CPI 调用，记录每笔 receipt 的 USDC 与 `CLAW`
  经济快照
- 维护用户和 provider 的奖励账户，区分 pending、locked、released、claimed
- 只在 attestation finalize 后释放 provider 待结算 USDC
- 在 challenge resolve 后结算 bond、refund、slash、burn 等经济结果
- 负责一次性的 `CLAW` genesis 库存铸造

## 高层模型

Phase 1 已经是按 receipt 驱动，而不是按 epoch 驱动。

当前职责边界是：

- `clawfarm-attestation` 负责 receipt / challenge 生命周期
- `clawfarm-masterpool` 负责所有代币流转和经济状态

在 receipt 记录时：

- payer 的 USDC 会立即拆分到 treasury 和 provider pending revenue
- user 和 provider 的 `CLAW` 奖励会立即以可回滚的 pending 余额入账
- 如果 provider 的 `claw_net_position` 为负，会先拿本次 provider 奖励冲抵负债

在 receipt finalize 时：

- pending user / provider 奖励会转入 locked 余额
- provider-share USDC 会从 pending vault 释放到 provider 钱包

在 challenge resolve 时：

- challenge 被驳回：销毁 challenger 的 bond
- challenge 被接受：先回滚被 challenge receipt 的 pending 奖励，再退还 bond、
  给 payer 退回 provider-share USDC、扣减 provider 的签名 `CLAW` 净头寸、
  从 reward inventory 支付 challenger 奖励，并销毁剩余部分

## 当前实现的关键约束

- Phase 1 使用的两个 token mint 都必须是 `6` 位精度
- `initialize_masterpool` 必须由当前程序 upgrade authority 通过
  `ProgramData` 账户授权
- provider 必须保持 `Active` 才能接收新的 receipt 结算；`Exited` 在
  Phase 1 中是终态
- basis point 的基数是经过 checked arithmetic 校验的 `1000`，更新配置时不会
  通过溢出绕过约束
- genesis mint 只能通过 `mint_genesis_supply` 执行一次
- `mint_genesis_supply` 会铸造 `1_000_000_000 * 10^6` `CLAW`，
  然后撤销 mint 和 freeze authority
- 每个 provider wallet 只对应一个 `ProviderAccount`
- 每个 attestation receipt 只对应一个 `ReceiptSettlement`
- 每个 attestation challenge 只对应一个 `ChallengeBondRecord`
- 只有配置里的 attestation program 所拥有的 config PDA 才能调用
  masterpool 的 attestation-only 接口
- challenge 被接受时，只退回 provider-share USDC，treasury-share 仍保留在
  treasury vault
- 奖励释放目前仍是管理员手动 helper，不是自动解锁调度器；但现在必须按单笔
  `ReceiptSettlement` 的锁仓快照和已过时间执行
- 奖励释放和 claim 只读取 locked 余额，pending 余额不可释放
- provider 的有符号奖励净头寸使用 checked `i128` 记账
- 超过 Phase 1 支持域的 receipt charge 会在记账入口被拒绝；会让后续数学溢出的治理参数也会在配置写入时 fail closed

## 治理参数

`Phase1ConfigParams` 当前包含：

- `exchange_rate_claw_per_usdc_e6`
- `provider_stake_usdc`
- `provider_usdc_share_bps`
- `treasury_usdc_share_bps`
- `user_claw_share_bps`
- `provider_claw_share_bps`
- `lock_days`
- `provider_slash_claw_amount`
- `challenger_reward_bps`
- `burn_bps`
- `challenge_bond_claw_amount`

## 程序状态

状态定义见 [src/state/accounts.rs](src/state/accounts.rs)。

### `GlobalConfig`

PDA seed：

- `["config"]`

保存：

- admin authority
- attestation program id
- `CLAW` 和 `USDC` mint 绑定
- 五个协议金库地址
- Phase 1 治理参数
- genesis mint 是否已执行
- receipt recording、challenge processing、finalization、claims 四类暂停开关

### `ProviderAccount`

PDA seed：

- `["provider", provider_wallet]`

保存：

- provider 钱包身份
- 已质押 USDC
- 待释放 provider-share USDC
- 有符号的 `claw_net_position`
- 未结算 receipt 数量
- 未解决 challenge 数量
- provider 状态

### `RewardAccount`

PDA seed：

- `["user_reward", user_wallet]`
- `["provider_reward", provider_wallet]`

保存：

- owner
- 账户类型（`User` 或 `Provider`）
- `pending_claw_total`
- `locked_claw_total`
- `released_claw_total`
- `claimed_claw_total`

### `ReceiptSettlement`

PDA seed：

- `["receipt_settlement", attestation_receipt]`

保存该笔 receipt 的不可变经济快照：

- payer user
- provider wallet
- 支付的 USDC 总额
- treasury-share USDC
- provider-share USDC
- user 奖励 `CLAW`
- provider 奖励 `CLAW`
- provider 负债抵扣量
- provider 实际锁定奖励量
- `lock_days_snapshot`
- `reward_lock_started_at`
- `user_claw_released`
- `provider_claw_released`
- settlement 状态

### `ChallengeBondRecord`

PDA seed：

- `["challenge_bond_record", attestation_challenge]`

保存 challenge 的经济快照：

- attestation receipt / challenge 身份
- challenger、payer、provider 身份
- 固定 bond 数量
- provider slash 快照
- challenger reward / burn 比例
- 预计算的 challenger reward / burn 数量
- bond 状态

## 状态枚举

定义见 [src/state/types.rs](src/state/types.rs)。

### `ProviderStatus`

- `0 = Active`
- `1 = Exited`

### `RewardAccountKind`

- `0 = User`
- `1 = Provider`

### `ReceiptSettlementStatus`

- `0 = Recorded`
- `1 = FinalizedSettled`
- `2 = ChallengedReverted`

### `ChallengeBondStatus`

- `0 = Locked`
- `1 = Returned`
- `2 = Burned`

## 金库布局

Masterpool 分别持有以下 PDA token account：

- `["reward_vault"]`：reward `CLAW`
- `["challenge_bond_vault"]`：challenge-bond `CLAW`
- `["treasury_usdc_vault"]`：treasury `USDC`
- `["provider_stake_usdc_vault"]`：provider stake `USDC`
- `["provider_pending_usdc_vault"]`：provider pending revenue `USDC`

统一的金库 authority PDA 是：

- `["pool_authority"]`

这样可以把 reward inventory、challenge collateral、provider escrow 和
treasury 资金完全隔离，便于审计和状态机控制。

## 接口总览

入口定义见 [src/lib.rs](src/lib.rs)。

### 管理员接口

- `initialize_masterpool(params: Phase1ConfigParams)`
  - 创建 `GlobalConfig` 和全部 vault PDA
  - 绑定 attestation program 和两个 token mint
  - 要求当前 upgrade authority 完成 bootstrap 授权
  - 校验 mint 精度、split invariant，以及支持的 receipt-charge 数学边界
- `mint_genesis_supply()`
  - 向 reward vault 铸造固定 genesis 供应量
  - 撤销 `CLAW` mint 的 mint / freeze authority
- `update_config(params: Phase1ConfigParams)`
  - 更新 Phase 1 的经济参数
- `set_pause_flags(pause_receipt_recording, pause_challenge_processing, pause_finalization, pause_claims)`
  - 切换四类运行时暂停开关

### Provider 接口

- `register_provider()`
  - 从 provider 钱包转入固定 USDC 质押
  - 创建 provider account 和 provider reward account
- `exit_provider()`
  - 只有在 pending USDC、未结算 receipt、未解决 challenge、负向
    `claw_net_position` 全部清空后，才退回质押

### Reward 接口

- `materialize_reward_release(target, amount)`
  - 管理员手动把某一笔已 finalized receipt 的 `User` 或 `Provider` 侧奖励，
    从 locked 挪到 released
  - 只允许释放该 receipt 在当前时点已经 vest 的 tranche；计算依据是
    `lock_days_snapshot`、`reward_lock_started_at` 和该 receipt 已释放计数
- `claim_released_claw()`
  - 把当前可领取的 `CLAW` 从 reward vault 转给 owner

### 仅供 Attestation 调用的经济接口

- `record_mining_from_receipt(args: { total_usdc_paid, charge_mint })`
  - 校验 attestation 调用方
  - 拒绝超出 Phase 1 支持结算域的 receipt charge
  - 要求 provider account 当前仍是 `Active`
  - 从 payer 收取 USDC
  - 拆分 treasury-share 和 provider-share
  - 按需初始化 reward account
  - 把 user / provider 奖励记录到 provisional pending 余额，并先处理
    provider 负债抵扣
  - 创建不可变的 `ReceiptSettlement`
- `settle_finalized_receipt(attestation_receipt_status)`
  - 校验 attestation 已 finalize 该 receipt
  - 把该 receipt 的 pending 奖励转入 locked 余额
  - 把 provider-share USDC 从 pending vault 转给 provider
  - 把 settlement 标记为 `FinalizedSettled`
- `record_challenge_bond()`
  - 把固定 `CLAW` bond 从 challenger 转入 bond vault
  - 快照 slash、challenger reward、burn 经济参数
- `resolve_challenge_economics(resolution_code)`
  - challenge 被驳回时销毁 bond
  - challenge 被接受时先回滚该 receipt 的 pending 奖励，再退回 bond、
    退款 provider-share USDC、更新 `claw_net_position`、支付 challenger reward，
    并销毁剩余部分

## Receipt 结算流程

1. `clawfarm-attestation::submit_receipt` 校验 canonical receipt 后，
   CPI 调用 `record_mining_from_receipt`。
2. Masterpool 把 payer 的 USDC 分别转入 treasury vault 和 provider pending vault。
3. Masterpool 把 user `CLAW` 奖励与 provider `CLAW` 奖励先记到 pending 余额；
   如果 provider 有负债，先冲抵负债。
4. Masterpool 以 attestation receipt PDA 为键创建 `ReceiptSettlement`。
5. 之后 attestation finalize 该 receipt，并 CPI 调用 `settle_finalized_receipt`。
6. Masterpool 把 pending 奖励转入 locked，释放 provider-share USDC，并把
   settlement 标记为 finalized。
7. 后续 reward release 必须再次引用同一个 `ReceiptSettlement`，并且只能释放这笔
   receipt 在其锁仓快照下已到期的部分。

## Challenge 流程

1. `clawfarm-attestation::open_challenge` CPI 调用 `record_challenge_bond`。
2. Masterpool 锁定固定 `CLAW` bond，并快照 challenge 经济参数。
3. 之后 `clawfarm-attestation::resolve_challenge` 再 CPI 调用
   `resolve_challenge_economics`。
4. 如果 challenge 被驳回：
   - 销毁 challenger bond
   - receipt 的经济记录保持有效
   - attestation 之后仍可 finalize 该 receipt，释放 provider payout
5. 如果 challenge 被接受、receipt invalidated 或 signer revoked：
   - 先回滚该 receipt 对应的 pending user / provider 奖励
   - 退回 challenger bond
   - 给 payer 退回 provider-share USDC
   - 扣减 provider 的有符号 `CLAW` 净头寸
   - 从 reward inventory 支付 challenger 奖励
   - 销毁剩余 slash 金额
   - 把 receipt settlement 标记为 `ChallengedReverted`

## 运维说明

- 两个程序都必须由当前 upgrade authority 完成 bootstrap；公开网络上随机首个调用者已不能夺取单例 config。
- `Exited` provider 在 Phase 1 中视为永久失活，不支持同一 PDA 重新入场。
- 奖励释放脚本只能读取 `locked_claw_total`。pending 余额在 attestation finalize 前故意保持可回滚。
- 扩宽后的 `claw_net_position` 和 reward account 布局属于 rollout 级状态变化；部署时应按 fresh rollout 或显式 migration 处理，而不是假设二进制兼容热升级。

## 已验证行为

当前端到端集成测试
[../../tests/phase1-integration.ts](../../tests/phase1-integration.ts) 覆盖：

- 未授权 bootstrap 初始化会被拒绝
- provider 注册
- 未授权直接调用 `record_mining_from_receipt` 失败
- `Exited` provider 继续收单会被拒绝
- 正常 receipt 记录、pending 奖励记账与 finalize
- 重复 receipt 防重
- challenge 被驳回后的 burn 路径以及后续 finalize
- challenge 被接受后的奖励回滚、退款与 slash 路径
- 治理参数越界/溢出校验
- 通过 [../../scripts/test-phase1.sh](../../scripts/test-phase1.sh)
  的确定性 bootstrap / deploy / test 路径
- user 奖励释放与领取
- provider 奖励释放与领取
- provider 在义务未清空前退出被阻止

## 开发

```bash
anchor build
yarn test
```
