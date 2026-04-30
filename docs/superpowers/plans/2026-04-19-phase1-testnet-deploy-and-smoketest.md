# Phase 1 测试网部署与验证执行文档

> 适用范围：`clawfarm-masterpool` / `clawfarm-attestation` 当前 `main` 分支的 fresh devnet rollout

## 1. 当前实现结论

- 当前 Phase 1 合约已经实现固定 `CLAW` mint + 固定 `Test USDC` mint 绑定，后续质押和 receipt 结算都只能使用 `GlobalConfig` 中记录的这两个 mint。
- `CLAW` 的 genesis mint 已固定为一次性铸造 `1_000_000_000 * 10^6` 基础单位，并要求先把 mint authority 和 freeze authority 转交给 `pool_authority`，再允许执行 `mint_genesis_supply`。
- `Test USDC` 的增发权限明确保留在合约外部 operator 钱包，不放入合约。
- 当前本地验证结果：
  - `./scripts/test-phase1.sh` 通过。
  - `tests/phase1-script-helpers.ts`、`tests/phase1-bootstrap-script.ts`、`tests/phase1-test-usdc-script.ts` 共 7 个脚本测试通过。
- 当前测试网验证缺口不在链上逻辑，而在 devnet 全生命周期 smoke test 还没有单独脚本化；执行时需要按 `tests/phase1-integration.ts` 的调用顺序做一次性手工验证或临时客户端验证。

## 2. 当前默认配置

### 2.1 Program ID

以 `Anchor.toml` 当前配置为准：

- `clawfarm_masterpool = AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux`
- `clawfarm_attestation = 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2`

### 2.2 Bootstrap 默认经济参数

以 `scripts/phase1/bootstrap-testnet.ts` 当前实现为准：

- `exchangeRateClawPerUsdcE6 = 1_000_000`
- `providerStakeUsdc = 100_000_000`
- `providerUsdcShareBps = 700`
- `treasuryUsdcShareBps = 300`
- `userClawShareBps = 300`
- `providerClawShareBps = 700`
- `lockDays = 180`
- `providerSlashClawAmount = 30_000_000`
- `challengerRewardBps = 700`
- `burnBps = 300`
- `challengeBondClawAmount = 2_000_000`

### 2.3 Attestation 时间窗口

以 `scripts/phase1/bootstrap-testnet.ts` 当前实现为准：

- `challenge_window_seconds = 86400`
- `challenge_resolution_timeout_seconds = 86400`

这意味着：

- 正常 receipt 的 `finalize_receipt` 需要在提交 24 小时后执行。
- `timeout_reject_challenge` 也需要在 challenge 打开 24 小时后执行。

### 2.4 部署记录文件

Bootstrap 输出文件默认路径：

- `deployments/devnet-phase1.json`

文件至少应包含以下字段：

- `clawMint`
- `testUsdcMint`
- `poolAuthority`
- `masterpoolConfig`
- `attestationConfig`
- `rewardVault`
- `challengeBondVault`
- `treasuryUsdcVault`
- `providerStakeUsdcVault`
- `providerPendingUsdcVault`
- `adminAuthority`
- `testUsdcOperator`

## 3. 部署前准备

### 3.1 环境要求

- Anchor `0.32.1`
- Solana CLI `3.1.12`
- Yarn `1.x`
- 一个 funded `admin` keypair
- 一个独立的 funded `test-usdc-operator` keypair

### 3.2 钱包职责

- `admin`
  - deploy 两个程序
  - 作为两个程序的 upgrade authority
  - 执行 bootstrap
  - 作为 attestation authority / pause authority / challenge resolver 的默认地址
- `test-usdc-operator`
  - 仅负责通过脚本增发测试稳定币
  - 不参与合约初始化

注意：

- `admin` 和 `test-usdc-operator` 不能使用同一个 keypair。
- 若 `admin` 不是 deploy 时的实际 upgrade authority，`initialize_masterpool` 和 `initialize_config` 会报 `UnauthorizedInitializer`。

### 3.3 准备命令

```bash
cd <clawfarm-masterpool-repo>

yarn install
solana config set --url https://api.devnet.solana.com

solana address -k <admin-keypair.json> ———— <admin-pubkey>
solana balance -k <admin-keypair.json> --url https://api.devnet.solana.com ———— <admin-sol-balance>

solana address -k <test-usdc-operator-keypair.json> ———— <test-usdc-operator-pubkey>
solana balance -k <test-usdc-operator-keypair.json> --url https://api.devnet.solana.com ———— <operator-sol-balance>
```

## 4. 上链前本地预检

执行以下命令，确认当前代码和脚本状态正常：

```bash
cd <clawfarm-masterpool-repo>

yarn test
npx ts-mocha -p ./tsconfig.json -t 1000000 \
  tests/phase1-script-helpers.ts \
  tests/phase1-bootstrap-script.ts \
  tests/phase1-test-usdc-script.ts
```

预期结果：

- `yarn test` 通过。
- 额外 `ts-mocha` 检查显示 `7 passing`。

## 5. 部署程序到 Devnet

### 5.1 构建

```bash
cd <clawfarm-masterpool-repo>

anchor build
```

### 5.2 部署 masterpool

```bash
solana program deploy \
  target/deploy/clawfarm_masterpool.so \
  --program-id target/deploy/clawfarm_masterpool-keypair.json \
  --upgrade-authority <admin-keypair.json> \
  --url https://api.devnet.solana.com
```

Program Id: AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux

### 5.3 部署 attestation

```bash
solana program deploy \
  target/deploy/clawfarm_attestation.so \
  --program-id target/deploy/clawfarm_attestation-keypair.json \
  --upgrade-authority <admin-keypair.json> \
  --url https://api.devnet.solana.com
```
Program Id: 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2

### 5.4 核对部署结果

```bash
solana program show AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux --url https://api.devnet.solana.com
solana program show 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2 --url https://api.devnet.solana.com
```

检查点：

- Program ID 与 `Anchor.toml` 一致。
- Upgrade authority 是预期的 `admin`。
- 部署返回的 tx signature 已记录。

## 6. Bootstrap 固定代币对

执行：

```bash
cd <clawfarm-masterpool-repo>

yarn phase1:bootstrap:testnet \
  --cluster devnet \
  --rpc-url "${SOLANA_RPC_URL}" \
  --admin-keypair <admin-keypair.json> \
  --test-usdc-operator-keypair <test-usdc-operator-keypair.json> \
  --masterpool-program-id AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux \
  --attestation-program-id 52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2 \
  --out deployments/devnet-phase1.json
```

该步骤将完成：

- 创建 `CLAW` mint，精度 `6`。
- 创建 `Test USDC` mint，精度 `6`。
- 将 `CLAW` mint authority 转移给 `pool_authority`。
- 将 `CLAW` freeze authority 转移给 `pool_authority`。
- 调用 `initialize_masterpool` 初始化 masterpool。
- 调用 `mint_genesis_supply` 向 `rewardVault` 铸造 genesis `CLAW`。
- 调用 `initialize_config` 初始化 attestation 配置。
- 输出完整部署记录到 `deployments/devnet-phase1.json`。

## 7. Bootstrap 后链上核对

### 7.1 核对部署记录文件

打开 `deployments/devnet-phase1.json`，确认以下字段均存在且非空：

- `clawMint`
- `testUsdcMint`
- `poolAuthority`
- `masterpoolConfig`
- `attestationConfig`
- `rewardVault`
- `challengeBondVault`
- `treasuryUsdcVault`
- `providerStakeUsdcVault`
- `providerPendingUsdcVault`
- `adminAuthority`
- `testUsdcOperator`

### 7.2 核对链上状态

必须确认：

- `clawMint` 精度为 `6`。
- `testUsdcMint` 精度为 `6`。
- `CLAW` mint authority 已不再保留在 admin。
- `CLAW` freeze authority 已不再保留在 admin。
- `rewardVault` 持有完整 `1_000_000_000 * 10^6` 基础单位 `CLAW`。
- `Test USDC` mint authority 仍指向 operator 钱包。
- `masterpoolConfig` 中记录的 `claw_mint` 与 `usdc_mint` 与部署记录一致。
- `attestationConfig` 中记录的 `masterpool_program` 与当前 masterpool program id 一致。

### 7.3 设计约束确认

确认后续所有业务都遵循：

- provider 质押只能使用 `testUsdcMint`。
- receipt 的 `charge_mint` 只能使用 `testUsdcMint`。
- 奖励领取只能从固定 `clawMint` 发放。
- 不能用任意第三方 mint 直接操作 Phase 1 合约。

## 8. 为测试角色注资

### 8.1 建议准备的角色

- `provider wallet` ———— <provider-wallet-pubkey>
- `provider signer`
- `payer wallet` ———— <payer-wallet-pubkey>
- `challenger wallet` ———— <challenger-wallet-pubkey>

所有角色都先准备少量 devnet SOL。

### 8.2 通过 operator 脚本增发 Test USDC

为 provider 注资：

```bash
cd <clawfarm-masterpool-repo>

yarn phase1:mint:test-usdc \
  --deployment deployments/devnet-phase1.json \
  --operator-keypair <test-usdc-operator-keypair.json> \
  --recipient <provider-wallet-pubkey> \
  --amount 1000
```

为 payer 或 challenger 注资：

```bash
yarn phase1:mint:test-usdc \
  --deployment deployments/devnet-phase1.json \
  --operator-keypair <test-usdc-operator-keypair.json> \
  --recipient <payer-or-challenger-wallet-pubkey> \
  --amount 30000
```

建议初始金额：

- provider：`1000`
- 普通 payer：`1000`
- 用于 challenge 测试的 payer / challenger：`30000+`

说明：

- `challenge_bond_claw_amount = 2 CLAW`。
- fresh deployment 下，外部钱包起初没有 `CLAW`，因此如果希望较快验证 challenge 流程，建议先让一个 payer 通过较大金额 receipt 拿到足够可释放的 `CLAW`。

## 9. 测试执行顺序

当前仓库没有单独的 devnet smoke-test 脚本，执行时请以 `tests/phase1-integration.ts` 为唯一调用顺序参考。

### 9.1 第 1 组：基础可用性与代币绑定

执行目标：

- 注册 provider signer。
- 注册 provider。

调用顺序：

1. 调 `upsert_provider_signer`。
2. 调 `register_provider`，使用绑定的 `Test USDC` mint 和 provider 的 Test USDC ATA。

预期结果：

- Provider 质押 `100 USDC` 成功。
- `ProviderAccount.status = Active`。

### 9.2 第 2 组：负向校验

#### 校验 A：错误质押币种

步骤：

1. 生成一个 rogue USDC mint。
2. 为备用 provider 创建 rogue mint 的 ATA。
3. 调 `register_provider` 时传入 rogue mint。

预期结果：

- 指令失败。
- 报错 `InvalidUsdcMint`。

#### 校验 B：错误 charge mint

步骤：

1. 构造一笔 receipt。
2. 将其 `charge_mint` 改成 rogue mint。
3. 调 `submit_receipt`。

预期结果：

- 指令失败。
- 报错 `ChargeMintMismatch`。

这两项验证通过后，说明固定代币绑定设计已经真实生效。

### 9.3 第 3 组：Happy Path receipt 提交

执行前提：

- provider signer 已注册。
- provider 已注册且处于 `Active`。
- payer 已持有足够 Test USDC。

步骤：

1. 按 `tests/phase1-integration.ts` 中 `submitReceipt` 的方式生成 receipt payload 和 `receipt_hash`。
2. 先插入 `ed25519` 验签指令。
3. 再调用 `submit_receipt`。

预期结果：

- 生成 attestation `Receipt` PDA。
- 生成 masterpool `ReceiptSettlement` PDA。
- payer 的 Test USDC 立即分到 `treasuryUsdcVault` 和 `providerPendingUsdcVault`。
- user/provider 的 `CLAW` 奖励先进入 pending 状态。

### 9.4 第 4 组：T+1 finalize 与奖励锁仓

步骤：

1. 等待 receipt 的 `challenge_window_seconds = 86400` 秒结束。
2. 调用 `finalize_receipt`。
3. 获取 `ReceiptSettlement`，检查 `lock_days_snapshot` 和 `reward_lock_started_at`。
4. 计算当前可释放奖励。
5. 调 `materialize_reward_release`。
6. 调 `claim_released_claw`。

预期结果：

- provider-share USDC 发放到 provider wallet。
- `ReceiptSettlement.status = FinalizedSettled`。
- pending 奖励转入 locked。
- 释放未超过可归属额度时成功。
- 超额释放时报 `RewardReleaseExceedsVested`。

### 9.5 第 5 组：Challenge rejected 路径

步骤：

1. 针对一笔新 receipt 调 `open_challenge`。
2. challenger 支付 `2 CLAW` bond。
3. resolver 调 `resolve_challenge(..., resolutionCode = 2)`。
4. 在经济尚未 finalize 前尝试关闭 receipt。
5. 再执行 `finalize_receipt`。

预期结果：

- challenge 被驳回。
- receipt 仍需后续 `finalize_receipt` 才完成经济结算。
- 若在 finalize 前 `close_receipt`，应报 `ReceiptEconomicsPending`。

### 9.6 第 6 组：Challenge accepted 路径

步骤：

1. 再提交一笔 receipt。
2. 执行 `open_challenge`。
3. resolver 调 `resolve_challenge(..., resolutionCode = 1)`。
4. 尝试对该 receipt 再做正常 finalize。

预期结果：

- 该笔 settlement 回滚为 challenged reverted。
- payer 仅拿回 provider-share USDC。
- treasury share 保留在 treasury vault。
- provider 发生 `CLAW` slash。
- challenger 获得奖励。
- 再做正常 finalize 应报 `ReceiptNotFinalizable`。

### 9.7 第 7 组：Timeout reject 路径

步骤：

1. 再提交一笔 receipt。
2. 调 `open_challenge`。
3. 不执行人工 resolve。
4. 等待 `opened_at + challenge_resolution_timeout_seconds`。
5. 由 authority 调 `timeout_reject_challenge`。
6. 再执行 `finalize_receipt`。

预期结果：

- challenge 状态变为 rejected。
- receipt 重新回到可 finalize 状态。
- provider payout 正常完成。

### 9.8 第 8 组：Provider exit 路径

步骤：

1. 确认 provider 的：
   - `pendingProviderUsdc = 0`
   - `unsettledReceiptCount = 0`
   - `unresolvedChallengeCount = 0`
2. 调 `exit_provider`。
3. 退出后再次尝试提交新 receipt 到该 provider。

预期结果：

- provider 成功取回 stake。
- `ProviderAccount.status = Exited`。
- 再提交新 receipt 报 `ProviderNotActive`。

## 10. 最小上线验收标准

以下项目全部完成后，视为“测试网上线基础可用”：

- 两个程序 deploy 成功。
- bootstrap 成功并生成 `deployments/devnet-phase1.json`。
- 链上确认 `CLAW` mint 与 `Test USDC` mint 已正确绑定进配置。
- provider signer 注册成功。
- provider 注册成功。
- 至少 1 笔正常 receipt 提交成功。
- 至少 1 笔正常 receipt 在 T+1 成功 finalize。

## 11. 完整功能验收标准

以下项目全部完成后，视为“Phase 1 测试网全流程通过”：

- `InvalidUsdcMint` 负向校验通过。
- `ChargeMintMismatch` 负向校验通过。
- 至少 1 次 reward release + claim 成功。
- 至少 1 次 challenge rejected 成功。
- 至少 1 次 challenge accepted 成功。
- 至少 1 次 timeout reject 成功。
- provider 最终可成功 exit。
- provider exit 后不能再接收新 receipt。

## 12. 必须归档的执行证据

至少保留以下内容：

- masterpool deploy tx signature
- attestation deploy tx signature
- bootstrap 过程主要 tx signatures
- provider signer 注册 tx signature
- provider 注册 tx signature
- 第一笔 receipt 提交 tx signature
- 第一笔 finalize tx signature
- challenge open / resolve / timeout tx signature
- `deployments/devnet-phase1.json` 原始文件

建议同时记录：

- 测试执行日期
- 使用的钱包地址
- 使用的 `clawMint` / `testUsdcMint`
- 每一步的链上结果截图或 explorer 链接

## 13. 执行注意事项

- 当前文档默认目标网络是 Solana devnet。
- 当前文档默认使用 `Anchor.toml` 中现有 program id，不替换 program 地址。
- 当前文档默认这是一次 fresh rollout，不包含历史部署迁移。
- 当前文档不要求新增代码、不要求新增自动化脚本、不要求提交仓库。
- 如果后续希望把第 9 节全部自动化，需要单独补一份 devnet smoke-test client。

## 14. 关键参考文件

- `docs/phase1-testnet-runbook.md`
- `scripts/phase1/bootstrap-testnet.ts`
- `scripts/phase1/mint-test-usdc.ts`
- `scripts/phase1/common.ts`
- `tests/phase1-integration.ts`
- `programs/clawfarm-masterpool/README.md`
- `programs/clawfarm-attestation/README.md`
