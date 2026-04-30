import * as anchor from "@coral-xyz/anchor";
import { assert } from "chai";
import {
  Ed25519Program,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  AuthorityType,
  approveChecked,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  setAuthority,
  TOKEN_PROGRAM_ID,
  transferChecked,
} from "@solana/spl-token";
import {
  buildCompactReceiptMetadata,
  buildCompactSubmitArgs,
  CompactReceiptMetadata,
  hashRequestNonce,
} from "../scripts/phase1/compact-receipt";

const USDC_UNIT = 1_000_000;
const CLAW_UNIT = 1_000_000;
const PROVIDER_STAKE_USDC = 100 * USDC_UNIT;
const RECEIPT_CHARGE_USDC = 10 * USDC_UNIT;
const CHALLENGE_BOND_CLAW = 2 * CLAW_UNIT;
const PROVIDER_SLASH_CLAW = 30 * CLAW_UNIT;
const PROVIDER_USDC_PER_RECEIPT = 7 * USDC_UNIT;
const TREASURY_USDC_PER_RECEIPT = 3 * USDC_UNIT;
const USER_REWARD_PER_RECEIPT = 3 * CLAW_UNIT;
const PROVIDER_REWARD_PER_RECEIPT = 7 * CLAW_UNIT;
const SEEDED_CHALLENGER_CLAW = 10 * CLAW_UNIT;
const FAUCET_PER_CLAIM = 10 * CLAW_UNIT;
const FAUCET_PER_WALLET_PER_DAY = 50 * CLAW_UNIT;
const FAUCET_GLOBAL_PER_DAY = 50_000 * CLAW_UNIT;
const CHALLENGE_WINDOW_SECONDS = 1;
const CHALLENGE_RESOLUTION_TIMEOUT_SECONDS = 1;
const UPGRADEABLE_LOADER_PROGRAM_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111"
);

describe("Phase 1 core economics", () => {
  const BN = ((anchor as any).BN ?? (anchor as any).default.BN) as typeof anchor.BN;
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const wallet = provider.wallet as anchor.Wallet;
  const masterpool = anchor.workspace.ClawfarmMasterpool as anchor.Program<any>;
  const attestation = anchor.workspace.ClawfarmAttestation as anchor.Program<any>;

  const attestationAuthority = wallet.publicKey;
  const pauseAuthority = wallet.publicKey;
  const challengeResolver = wallet.publicKey;
  const providerCode = "u";
  const providerSigner = Keypair.generate();
  const providerWallet = Keypair.generate();
  const alternateProviderWallet = Keypair.generate();
  const payerUser = Keypair.generate();
  const alternatePayerUser = Keypair.generate();
  const feePayer = Keypair.generate();
  const paymentDelegate = Keypair.generate();
  const wrongPaymentDelegate = Keypair.generate();

  let clawMint: PublicKey;
  let usdcMint: PublicKey;
  let rogueUsdcMint: PublicKey;
  let masterpoolConfigPda: PublicKey;
  let rewardVaultPda: PublicKey;
  let challengeBondVaultPda: PublicKey;
  let treasuryUsdcVaultPda: PublicKey;
  let providerStakeVaultPda: PublicKey;
  let providerPendingVaultPda: PublicKey;
  let poolAuthorityPda: PublicKey;
  let faucetConfigPda: PublicKey;
  let faucetGlobalPda: PublicKey;
  let faucetUserPda: PublicKey;
  let faucetClawVaultPda: PublicKey;
  let faucetUsdcVaultPda: PublicKey;
  let faucetUser = Keypair.generate();
  let faucetUserClawAta: PublicKey;
  let faucetUserUsdcAta: PublicKey;
  let attestationConfigPda: PublicKey;
  let providerSignerPda: PublicKey;
  let alternateProviderSignerPda: PublicKey;
  let providerAccountPda: PublicKey;
  let providerRewardPda: PublicKey;
  let userRewardPda: PublicKey;
  let alternateUserRewardPda: PublicKey;
  let masterpoolProgramData: PublicKey;
  let attestationProgramData: PublicKey;

  let providerUsdcAta: PublicKey;
  let alternateProviderUsdcAta: PublicKey;
  let alternateProviderRogueUsdcAta: PublicKey;
  let payerUsdcAta: PublicKey;
  let alternatePayerUsdcAta: PublicKey;
  let payerClawAta: PublicKey;
  let providerClawAta: PublicKey;
  let alternateProviderAccountPda: PublicKey;
  let alternateProviderRewardPda: PublicKey;

  before(async () => {
    [masterpoolConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      masterpool.programId
    );
    [rewardVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_vault")],
      masterpool.programId
    );
    [challengeBondVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("challenge_bond_vault")],
      masterpool.programId
    );
    [treasuryUsdcVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury_usdc_vault")],
      masterpool.programId
    );
    [providerStakeVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider_stake_usdc_vault")],
      masterpool.programId
    );
    [providerPendingVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider_pending_usdc_vault")],
      masterpool.programId
    );
    [poolAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_authority")],
      masterpool.programId
    );
    [faucetConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_config")],
      masterpool.programId
    );
    [faucetGlobalPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_global")],
      masterpool.programId
    );
    [faucetClawVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_claw_vault")],
      masterpool.programId
    );
    [faucetUsdcVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_usdc_vault")],
      masterpool.programId
    );
    [faucetUserPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), faucetUser.publicKey.toBuffer()],
      masterpool.programId
    );
    [attestationConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      attestation.programId
    );
    [providerSignerPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("provider_signer"),
        providerWallet.publicKey.toBuffer(),
        providerSigner.publicKey.toBuffer(),
      ],
      attestation.programId
    );
    [alternateProviderSignerPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("provider_signer"),
        alternateProviderWallet.publicKey.toBuffer(),
        providerSigner.publicKey.toBuffer(),
      ],
      attestation.programId
    );
    [providerAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), providerWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [alternateProviderAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), alternateProviderWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [providerRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider_reward"), providerWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [alternateProviderRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider_reward"), alternateProviderWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [userRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_reward"), payerUser.publicKey.toBuffer()],
      masterpool.programId
    );
    [alternateUserRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_reward"), alternatePayerUser.publicKey.toBuffer()],
      masterpool.programId
    );
    masterpoolProgramData = deriveProgramDataAddress(masterpool.programId);
    attestationProgramData = deriveProgramDataAddress(attestation.programId);

    await airdrop(providerWallet.publicKey);
    await airdrop(alternateProviderWallet.publicKey);
    await airdrop(payerUser.publicKey);
    await airdrop(alternatePayerUser.publicKey);
    await airdrop(feePayer.publicKey);
    await airdrop(paymentDelegate.publicKey);
    await airdrop(wrongPaymentDelegate.publicKey);
    await airdrop(faucetUser.publicKey);

    clawMint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      wallet.publicKey,
      6
    );
    usdcMint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      6
    );
    rogueUsdcMint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      6
    );

    providerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        providerWallet.publicKey
      )
    ).address;
    alternateProviderUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        alternateProviderWallet.publicKey
      )
    ).address;
    alternateProviderRogueUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        rogueUsdcMint,
        alternateProviderWallet.publicKey
      )
    ).address;
    payerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        payerUser.publicKey
      )
    ).address;
    alternatePayerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        alternatePayerUser.publicKey
      )
    ).address;
    payerClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        payerUser.publicKey
      )
    ).address;
    providerClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        providerWallet.publicKey
      )
    ).address;
    faucetUserClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        faucetUser.publicKey
      )
    ).address;
    faucetUserUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        faucetUser.publicKey
      )
    ).address;

    await mintTo(
      provider.connection,
      wallet.payer,
      clawMint,
      payerClawAta,
      wallet.publicKey,
      SEEDED_CHALLENGER_CLAW
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      providerUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      alternateProviderUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      rogueUsdcMint,
      alternateProviderRogueUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      payerUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      alternatePayerUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );

  });

  it("executes the Phase 1 receipt, challenge, and reward lifecycle", async () => {
    await expectAnchorError(
      masterpool.methods
        .initializeMasterpool({
          exchangeRateClawPerUsdcE6: new BN(CLAW_UNIT),
          providerStakeUsdc: new BN(PROVIDER_STAKE_USDC),
          providerUsdcShareBps: 700,
          treasuryUsdcShareBps: 300,
          userClawShareBps: 300,
          providerClawShareBps: 700,
          lockDays: 180,
          providerSlashClawAmount: new BN(PROVIDER_SLASH_CLAW),
          challengerRewardBps: 700,
          burnBps: 300,
          challengeBondClawAmount: new BN(CHALLENGE_BOND_CLAW),
        })
        .accounts({
          config: masterpoolConfigPda,
          rewardVault: rewardVaultPda,
          challengeBondVault: challengeBondVaultPda,
          treasuryUsdcVault: treasuryUsdcVaultPda,
          providerStakeUsdcVault: providerStakeVaultPda,
          providerPendingUsdcVault: providerPendingVaultPda,
          clawMint,
          usdcMint,
          attestationProgram: attestation.programId,
          selfProgram: masterpool.programId,
          selfProgramData: masterpoolProgramData,
          poolAuthority: poolAuthorityPda,
          initializer: providerWallet.publicKey,
          admin: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([providerWallet])
        .rpc(),
      "UnauthorizedInitializer"
    );

    await expectAnchorError(
      attestation.methods
        .initializeConfig(
          attestationAuthority,
          pauseAuthority,
          challengeResolver,
          masterpool.programId,
          new BN(CHALLENGE_WINDOW_SECONDS),
          new BN(CHALLENGE_RESOLUTION_TIMEOUT_SECONDS)
        )
        .accounts({
          initializer: providerWallet.publicKey,
          config: attestationConfigPda,
          selfProgram: attestation.programId,
          selfProgramData: attestationProgramData,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([providerWallet])
        .rpc(),
      "UnauthorizedInitializer"
    );

    await masterpool.methods
      .initializeMasterpool({
        exchangeRateClawPerUsdcE6: new BN(CLAW_UNIT),
        providerStakeUsdc: new BN(PROVIDER_STAKE_USDC),
        providerUsdcShareBps: 700,
        treasuryUsdcShareBps: 300,
        userClawShareBps: 300,
        providerClawShareBps: 700,
        lockDays: 180,
        providerSlashClawAmount: new BN(PROVIDER_SLASH_CLAW),
        challengerRewardBps: 700,
        burnBps: 300,
        challengeBondClawAmount: new BN(CHALLENGE_BOND_CLAW),
      })
      .accounts({
        config: masterpoolConfigPda,
        rewardVault: rewardVaultPda,
        challengeBondVault: challengeBondVaultPda,
        treasuryUsdcVault: treasuryUsdcVaultPda,
        providerStakeUsdcVault: providerStakeVaultPda,
        providerPendingUsdcVault: providerPendingVaultPda,
        clawMint,
        usdcMint,
        attestationProgram: attestation.programId,
        selfProgram: masterpool.programId,
        selfProgramData: masterpoolProgramData,
        poolAuthority: poolAuthorityPda,
        initializer: wallet.publicKey,
        admin: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    await expectAnchorError(
      masterpool.methods
        .mintGenesisSupply()
        .accounts({
          config: masterpoolConfigPda,
          adminAuthority: wallet.publicKey,
          rewardVault: rewardVaultPda,
          clawMint,
          poolAuthority: poolAuthorityPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        } as any)
        .rpc(),
      "InvalidClawMintAuthority"
    );

    await setAuthority(
      provider.connection,
      wallet.payer,
      clawMint,
      wallet.payer,
      AuthorityType.MintTokens,
      poolAuthorityPda
    );
    await setAuthority(
      provider.connection,
      wallet.payer,
      clawMint,
      wallet.payer,
      AuthorityType.FreezeAccount,
      poolAuthorityPda
    );

    await masterpool.methods
      .mintGenesisSupply()
      .accounts({
        config: masterpoolConfigPda,
        adminAuthority: wallet.publicKey,
        rewardVault: rewardVaultPda,
        clawMint,
        poolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();

    await attestation.methods
      .initializeConfig(
        attestationAuthority,
        pauseAuthority,
        challengeResolver,
        masterpool.programId,
        new BN(CHALLENGE_WINDOW_SECONDS),
        new BN(CHALLENGE_RESOLUTION_TIMEOUT_SECONDS)
      )
      .accounts({
        initializer: wallet.publicKey,
        config: attestationConfigPda,
        selfProgram: attestation.programId,
        selfProgramData: attestationProgramData,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    await attestation.methods
      .upsertProviderSigner(
        providerSigner.publicKey,
        providerWallet.publicKey,
        1 << 1,
        new BN(0),
        new BN(0)
      )
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        providerSigner: providerSignerPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    await expectAnchorError(
      masterpool.methods
        .registerProvider()
        .accounts({
          config: masterpoolConfigPda,
          providerAccount: alternateProviderAccountPda,
          providerRewardAccount: alternateProviderRewardPda,
          providerWallet: alternateProviderWallet.publicKey,
          providerStakeUsdcVault: providerStakeVaultPda,
          providerUsdcToken: alternateProviderRogueUsdcAta,
          usdcMint: rogueUsdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([alternateProviderWallet])
        .rpc(),
      "InvalidUsdcMint"
    );

    await masterpool.methods
      .registerProvider()
      .accounts({
        config: masterpoolConfigPda,
        providerAccount: providerAccountPda,
        providerRewardAccount: providerRewardPda,
        providerWallet: providerWallet.publicKey,
        providerStakeUsdcVault: providerStakeVaultPda,
        providerUsdcToken: providerUsdcAta,
        usdcMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([providerWallet])
      .rpc();

    await masterpool.methods
      .registerProvider()
      .accounts({
        config: masterpoolConfigPda,
        providerAccount: alternateProviderAccountPda,
        providerRewardAccount: alternateProviderRewardPda,
        providerWallet: alternateProviderWallet.publicKey,
        providerStakeUsdcVault: providerStakeVaultPda,
        providerUsdcToken: alternateProviderUsdcAta,
        usdcMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([alternateProviderWallet])
      .rpc();

    await expectAnchorError(
      submitReceipt("mint-mismatch", {
        usdcMintOverride: rogueUsdcMint,
        skipDelegateApproval: true,
      }),
      "InvalidUsdcMint"
    );

    const unauthorizedReceipt = Keypair.generate().publicKey;
    const unauthorizedSettlement = deriveReceiptSettlementPda(unauthorizedReceipt);
    await expectAnchorError(
      masterpool.methods
        .recordMiningFromReceipt({
          totalUsdcPaid: new BN(RECEIPT_CHARGE_USDC),
        })
        .accounts({
          config: masterpoolConfigPda,
          attestationConfig: attestationConfigPda,
          payerUser: payerUser.publicKey,
          feePayer: feePayer.publicKey,
          paymentDelegate: paymentDelegate.publicKey,
          payerUsdcToken: payerUsdcAta,
          providerAccount: providerAccountPda,
          providerRewardAccount: providerRewardPda,
          userRewardAccount: userRewardPda,
          receiptSettlement: unauthorizedSettlement,
          attestationReceipt: unauthorizedReceipt,
          treasuryUsdcVault: treasuryUsdcVaultPda,
          providerPendingUsdcVault: providerPendingVaultPda,
          usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([feePayer, paymentDelegate])
        .rpc(),
      "Signature verification failed"
    );

    await expectAnchorError(
      submitReceipt("sp", {
        providerAccount: alternateProviderAccountPda,
        providerRewardAccount: alternateProviderRewardPda,
      }),
      "ReceiptIdentityMismatch"
    );

    const receipt1 = await submitReceipt("r1", {
      metadata: {
        proofId: `proof-${"x".repeat(512)}`,
        providerCode: `gateway/${"y".repeat(256)}`,
        model: `model-${"z".repeat(512)}`,
      },
    });
    assert.match(receipt1.signature, /^[1-9A-HJ-NP-Za-km-z]{32,}$/);
    await waitForReceiptFinalizable(receipt1.receiptPda);
    await finalizeReceipt(receipt1.receiptPda, receipt1.settlementPda);

    const receipt1Settlement = await masterpool.account.receiptSettlement.fetch(
      receipt1.settlementPda
    );
    assert.equal(receipt1Settlement.usdcToProvider.toNumber(), PROVIDER_USDC_PER_RECEIPT);
    assert.equal(receipt1Settlement.usdcToTreasury.toNumber(), TREASURY_USDC_PER_RECEIPT);
    assert.equal(receipt1Settlement.lockDaysSnapshot, 180);
    assert.isAbove(decodeI64(receipt1Settlement.rewardLockStartedAt), 0);

    await expectAnchorError(
      materializeRewardRelease(
        userRewardPda,
        receipt1.settlementPda,
        0,
        USER_REWARD_PER_RECEIPT
      ),
      "RewardReleaseExceedsVested"
    );

    await waitUntilAfter(decodeI64(receipt1Settlement.rewardLockStartedAt) + 6);

    const userReleaseAmount = computeLinearReleasableAmount(
      decodeU64(receipt1Settlement.clawToUser),
      decodeU64(receipt1Settlement.userClawReleased),
      decodeI64(receipt1Settlement.rewardLockStartedAt),
      await currentUnixTime(),
      receipt1Settlement.lockDaysSnapshot
    );
    assert.isAbove(userReleaseAmount, 0);
    await releaseAndClaimUser(receipt1.settlementPda, userReleaseAmount);

    const providerReleaseAmount = computeLinearReleasableAmount(
      decodeU64(receipt1Settlement.clawToProviderLocked),
      decodeU64(receipt1Settlement.providerClawReleased),
      decodeI64(receipt1Settlement.rewardLockStartedAt),
      await currentUnixTime(),
      receipt1Settlement.lockDaysSnapshot
    );
    assert.isAbove(providerReleaseAmount, 0);
    await materializeRewardRelease(
      providerRewardPda,
      receipt1.settlementPda,
      1,
      providerReleaseAmount
    );
    await claimProviderReleasedClaw();

    assert.equal(
      Number((await getAccount(provider.connection, payerClawAta)).amount),
      SEEDED_CHALLENGER_CLAW + userReleaseAmount
    );
    assert.equal(
      Number((await getAccount(provider.connection, providerClawAta)).amount),
      providerReleaseAmount
    );

    const receipt2 = await submitReceipt("r2");
    const challenge2 = await openChallenge(receipt2);
    assert.equal(
      Number((await getAccount(provider.connection, payerClawAta)).amount),
      SEEDED_CHALLENGER_CLAW + userReleaseAmount - CHALLENGE_BOND_CLAW
    );
    await resolveChallenge(receipt2, challenge2, 2);
    await expectAnchorError(
      attestation.methods
        .closeReceipt()
        .accounts({
          authority: wallet.publicKey,
          config: attestationConfigPda,
          receipt: receipt2.receiptPda,
        } as any)
        .rpc(),
      "ReceiptEconomicsPending"
    );
    await finalizeReceipt(receipt2.receiptPda, receipt2.settlementPda);

    const receipt3 = await submitReceipt("r3");
    const challenge3 = await openChallenge(receipt3);
    await resolveChallenge(receipt3, challenge3, 1);
    assert.equal(
      Number((await getAccount(provider.connection, payerClawAta)).amount),
      SEEDED_CHALLENGER_CLAW + userReleaseAmount + 19 * CLAW_UNIT
    );

    await expectAnchorError(
      materializeRewardRelease(
        userRewardPda,
        receipt3.settlementPda,
        0,
        USER_REWARD_PER_RECEIPT
      ),
      "InvalidReceiptSettlementState"
    );

    const userRewardStateAfterReceipt3 = await masterpool.account.rewardAccount.fetch(
      userRewardPda
    );
    assert.equal(userRewardStateAfterReceipt3.pendingClawTotal.toNumber(), 0);
    assert.equal(
      userRewardStateAfterReceipt3.lockedClawTotal.toNumber(),
      2 * USER_REWARD_PER_RECEIPT - userReleaseAmount
    );
    assert.equal(
      userRewardStateAfterReceipt3.releasedClawTotal.toNumber(),
      userReleaseAmount
    );
    assert.equal(
      userRewardStateAfterReceipt3.claimedClawTotal.toNumber(),
      userReleaseAmount
    );

    const providerRewardStateAfterReceipt3 = await masterpool.account.rewardAccount.fetch(
      providerRewardPda
    );
    assert.equal(providerRewardStateAfterReceipt3.pendingClawTotal.toNumber(), 0);
    assert.equal(
      providerRewardStateAfterReceipt3.lockedClawTotal.toNumber(),
      2 * PROVIDER_REWARD_PER_RECEIPT - providerReleaseAmount
    );
    assert.equal(
      providerRewardStateAfterReceipt3.releasedClawTotal.toNumber(),
      providerReleaseAmount
    );
    assert.equal(
      providerRewardStateAfterReceipt3.claimedClawTotal.toNumber(),
      providerReleaseAmount
    );

    await expectAnchorError(
      attestation.methods
        .finalizeReceipt()
        .accounts({
          authority: wallet.publicKey,
          config: attestationConfigPda,
          receipt: receipt3.receiptPda,
          masterpoolConfig: masterpoolConfigPda,
          masterpoolProgram: masterpool.programId,
          masterpoolReceiptSettlement: receipt3.settlementPda,
          masterpoolProviderAccount: providerAccountPda,
          masterpoolProviderRewardAccount: providerRewardPda,
          masterpoolUserRewardAccount: userRewardPda,
          masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
          masterpoolProviderDestinationUsdc: providerUsdcAta,
          usdcMint,
          masterpoolPoolAuthority: poolAuthorityPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        } as any)
        .rpc(),
      "ReceiptNotFinalizable"
    );

    await expectAnchorError(
      masterpool.methods
        .exitProvider()
        .accounts({
          config: masterpoolConfigPda,
          providerAccount: providerAccountPda,
          providerWallet: providerWallet.publicKey,
          providerStakeUsdcVault: providerStakeVaultPda,
          providerDestinationUsdc: providerUsdcAta,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        } as any)
        .signers([providerWallet])
        .rpc(),
      "ProviderExitBlocked"
    );

    const receipt4 = await submitReceipt("r4");
    await expectAnchorError(submitReceipt("r4"), "already in use");
    await waitForReceiptFinalizable(receipt4.receiptPda);
    await finalizeReceipt(receipt4.receiptPda, receipt4.settlementPda);

    const receipt5 = await submitReceipt("r5");
    await waitForReceiptFinalizable(receipt5.receiptPda);
    await finalizeReceipt(receipt5.receiptPda, receipt5.settlementPda);

    const receipt6 = await submitReceipt("r6");
    await waitForReceiptFinalizable(receipt6.receiptPda);
    await finalizeReceipt(receipt6.receiptPda, receipt6.settlementPda);

    const receipt7 = await submitReceipt("r7");
    const challenge7 = await openChallenge(receipt7);
    await waitForChallengeTimeout(challenge7.challengePda);
    await timeoutRejectChallenge(receipt7, challenge7);
    await finalizeReceipt(receipt7.receiptPda, receipt7.settlementPda);

    const timeoutChallengeState = await attestation.account.challenge.fetch(
      challenge7.challengePda
    );
    assert.equal(timeoutChallengeState.status, 2);
    assert.equal(
      Number((await getAccount(provider.connection, payerClawAta)).amount),
      SEEDED_CHALLENGER_CLAW + userReleaseAmount + 17 * CLAW_UNIT
    );

    await masterpool.methods
      .exitProvider()
      .accounts({
        config: masterpoolConfigPda,
        providerAccount: providerAccountPda,
        providerWallet: providerWallet.publicKey,
        providerStakeUsdcVault: providerStakeVaultPda,
        providerDestinationUsdc: providerUsdcAta,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([providerWallet])
      .rpc();

    await expectAnchorError(
      submitReceipt("r8"),
      "ProviderNotActive"
    );

    const providerState = await masterpool.account.providerAccount.fetch(providerAccountPda);
    assert.equal(providerState.status, 1);
    assert.equal(providerState.pendingProviderUsdc.toNumber(), 0);
    assert.equal(providerState.unsettledReceiptCount.toNumber(), 0);
    assert.equal(providerState.unresolvedChallengeCount.toNumber(), 0);
    assert.equal(providerState.clawNetPosition.toNumber(), 12_000_000);

    const settlement3 = await masterpool.account.receiptSettlement.fetch(
      receipt3.settlementPda
    );
    assert.equal(settlement3.status, 2);

    const challengeRecord3 = await masterpool.account.challengeBondRecord.fetch(
      challenge3.challengeBondRecordPda
    );
    assert.equal(challengeRecord3.status, 1);

    const expectedFinalPayerUsdc =
      1_000 * USDC_UNIT - 7 * RECEIPT_CHARGE_USDC + PROVIDER_USDC_PER_RECEIPT;
    assert.equal(
      Number((await getAccount(provider.connection, payerUsdcAta)).amount),
      expectedFinalPayerUsdc
    );
  });


  it("initializes, funds, enables, and enforces the devnet faucet", async () => {
    await ensureFaucetBootstrapped({ enabled: false, clawVaultBalance: FAUCET_PER_CLAIM });

    const initialConfig = await (masterpool.account as any).faucetConfig.fetch(
      faucetConfigPda
    );
    assert.equal(initialConfig.enabled, false);
    assert.equal(initialConfig.maxClawPerClaim.toNumber(), FAUCET_PER_CLAIM);
    assert.equal(initialConfig.maxUsdcPerClaim.toNumber(), FAUCET_PER_CLAIM);
    assert.equal(
      initialConfig.maxClawPerWalletPerDay.toNumber(),
      FAUCET_PER_WALLET_PER_DAY
    );
    assert.equal(
      initialConfig.maxUsdcPerWalletPerDay.toNumber(),
      FAUCET_PER_WALLET_PER_DAY
    );
    assert.equal(
      initialConfig.maxClawGlobalPerDay.toNumber(),
      FAUCET_GLOBAL_PER_DAY
    );
    assert.equal(
      initialConfig.maxUsdcGlobalPerDay.toNumber(),
      FAUCET_GLOBAL_PER_DAY
    );

    const rewardVaultBefore = await getAccount(provider.connection, rewardVaultPda);
    const faucetClawBefore = await getAccount(provider.connection, faucetClawVaultPda);
    await masterpool.methods
      .fundFaucetClaw(new BN(CLAW_UNIT))
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        rewardVault: rewardVaultPda,
        faucetClawVault: faucetClawVaultPda,
        clawMint,
        poolAuthority: poolAuthorityPda,
        adminAuthority: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();
    const rewardVaultAfter = await getAccount(provider.connection, rewardVaultPda);
    const faucetClawAfter = await getAccount(provider.connection, faucetClawVaultPda);
    assert.equal(
      rewardVaultAfter.amount.toString(),
      (rewardVaultBefore.amount - BigInt(CLAW_UNIT)).toString()
    );
    assert.equal(
      faucetClawAfter.amount.toString(),
      (faucetClawBefore.amount + BigInt(CLAW_UNIT)).toString()
    );

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(1 * CLAW_UNIT),
          usdcAmount: new BN(1 * USDC_UNIT),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc(),
      "FaucetDisabled"
    );

    await masterpool.methods
      .setFaucetEnabled(true)
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        adminAuthority: wallet.publicKey,
      } as any)
      .rpc();

    await masterpool.methods
      .claimFaucet({
        clawAmount: new BN(10 * CLAW_UNIT),
        usdcAmount: new BN(5 * USDC_UNIT),
      })
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        faucetGlobalState: faucetGlobalPda,
        faucetUserState: faucetUserPda,
        faucetClawVault: faucetClawVaultPda,
        faucetUsdcVault: faucetUsdcVaultPda,
        userClawToken: faucetUserClawAta,
        userUsdcToken: faucetUserUsdcAta,
        clawMint,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        user: faucetUser.publicKey,
        payer: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    const clawAccount = await getAccount(provider.connection, faucetUserClawAta);
    const usdcAccount = await getAccount(provider.connection, faucetUserUsdcAta);
    assert.equal(clawAccount.amount.toString(), String(10 * CLAW_UNIT));
    assert.equal(usdcAccount.amount.toString(), String(5 * USDC_UNIT));

    const userState = await (masterpool.account as any).faucetUserState.fetch(
      faucetUserPda
    );
    assert.equal(userState.owner.toBase58(), faucetUser.publicKey.toBase58());
    assert.equal(userState.clawClaimedToday.toNumber(), 10 * CLAW_UNIT);
    assert.equal(userState.usdcClaimedToday.toNumber(), 5 * USDC_UNIT);

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(11 * CLAW_UNIT),
          usdcAmount: new BN(0),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc(),
      "FaucetClaimLimitExceeded"
    );
  });

  it("lets a fee payer claim faucet tokens for a recipient without recipient signature", async () => {
    await ensureFaucetBootstrapped({
      enabled: true,
      clawVaultBalance: 1 * CLAW_UNIT,
      usdcVaultBalance: FAUCET_PER_WALLET_PER_DAY,
    });

    const recipient = Keypair.generate();
    const recipientClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        recipient.publicKey
      )
    ).address;
    const recipientUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        recipient.publicKey
      )
    ).address;
    const [recipientFaucetState] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), recipient.publicKey.toBuffer()],
      masterpool.programId
    );

    await masterpool.methods
      .claimFaucet({
        clawAmount: new BN(1 * CLAW_UNIT),
        usdcAmount: new BN(2 * USDC_UNIT),
      })
      .accounts({
        config: masterpoolConfigPda,
        faucetConfig: faucetConfigPda,
        faucetGlobalState: faucetGlobalPda,
        faucetUserState: recipientFaucetState,
        faucetClawVault: faucetClawVaultPda,
        faucetUsdcVault: faucetUsdcVaultPda,
        userClawToken: recipientClawAta,
        userUsdcToken: recipientUsdcAta,
        clawMint,
        usdcMint,
        poolAuthority: poolAuthorityPda,
        user: recipient.publicKey,
        payer: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    const clawAccount = await getAccount(provider.connection, recipientClawAta);
    const usdcAccount = await getAccount(provider.connection, recipientUsdcAta);
    const userState = await (masterpool.account as any).faucetUserState.fetch(
      recipientFaucetState
    );

    assert.equal(clawAccount.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(usdcAccount.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(clawAccount.amount.toString(), String(1 * CLAW_UNIT));
    assert.equal(usdcAccount.amount.toString(), String(2 * USDC_UNIT));
    assert.equal(userState.owner.toBase58(), recipient.publicKey.toBase58());
    assert.equal(userState.clawClaimedToday.toNumber(), 1 * CLAW_UNIT);
    assert.equal(userState.usdcClaimedToday.toNumber(), 2 * USDC_UNIT);
  });

  it("rejects invalid faucet admin and claim operations", async () => {
    await ensureFaucetBootstrapped({ enabled: true });

    await expectAnchorError(
      masterpool.methods
        .setFaucetEnabled(false)
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          adminAuthority: faucetUser.publicKey,
        } as any)
        .signers([faucetUser])
        .rpc(),
      "UnauthorizedAdmin"
    );

    await expectAnchorError(
      masterpool.methods
        .updateFaucetLimits({
          maxClawPerClaim: new BN(0),
          maxUsdcPerClaim: new BN(10 * USDC_UNIT),
          maxClawPerWalletPerDay: new BN(50 * CLAW_UNIT),
          maxUsdcPerWalletPerDay: new BN(50 * USDC_UNIT),
          maxClawGlobalPerDay: new BN(FAUCET_GLOBAL_PER_DAY),
          maxUsdcGlobalPerDay: new BN(FAUCET_GLOBAL_PER_DAY),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          adminAuthority: wallet.publicKey,
        } as any)
        .rpc(),
      "InvalidFaucetLimits"
    );

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({ clawAmount: new BN(0), usdcAmount: new BN(0) })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: faucetUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: faucetUserClawAta,
          userUsdcToken: faucetUserUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: faucetUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc(),
      "InvalidFaucetAmount"
    );
  });

  it("enforces the faucet per-wallet daily limit", async () => {
    await ensureFaucetBootstrapped({ enabled: true, usdcVaultBalance: 51 * USDC_UNIT });

    const limitedUser = Keypair.generate();
    await airdrop(limitedUser.publicKey);
    const limitedClawAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        clawMint,
        limitedUser.publicKey
      )
    ).address;
    const limitedUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        limitedUser.publicKey
      )
    ).address;
    const [limitedUserPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faucet_user"), limitedUser.publicKey.toBuffer()],
      masterpool.programId
    );

    for (let index = 0; index < 5; index += 1) {
      await masterpool.methods
        .claimFaucet({
          clawAmount: new BN(0),
          usdcAmount: new BN(10 * USDC_UNIT),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: limitedUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: limitedClawAta,
          userUsdcToken: limitedUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: limitedUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc();
    }

    await expectAnchorError(
      masterpool.methods
        .claimFaucet({
          clawAmount: new BN(0),
          usdcAmount: new BN(1 * USDC_UNIT),
        })
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetUserState: limitedUserPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          userClawToken: limitedClawAta,
          userUsdcToken: limitedUsdcAta,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          user: limitedUser.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc(),
      "FaucetWalletDailyLimitExceeded"
    );
  });

  it("settles browser wallet receipts with fee payer and payment delegate while payer_user does not sign", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance({
      amount: RECEIPT_CHARGE_USDC,
      delegate: feePayer.publicKey,
      mint: usdcMint,
      owner: payerUser,
      tokenAccount: payerUsdcAta,
    });
    const payerLamportsBefore = await provider.connection.getBalance(payerUser.publicKey);
    const feePayerLamportsBefore = await provider.connection.getBalance(feePayer.publicKey);
    const payerUsdcBefore = await getAccount(provider.connection, payerUsdcAta);

    const receipt = await submitReceipt("delegate-success", {
      ...activeAlternateProviderOverrides(),
      skipDelegateApproval: true,
    });

    const payerLamportsAfter = await provider.connection.getBalance(payerUser.publicKey);
    const feePayerLamportsAfter = await provider.connection.getBalance(feePayer.publicKey);
    const payerUsdcAfter = await getAccount(provider.connection, payerUsdcAta);
    const receiptState = await attestation.account.receipt.fetch(receipt.receiptPda);
    const settlementState = await masterpool.account.receiptSettlement.fetch(receipt.settlementPda);

    assert.equal(receiptState.payerUser.toBase58(), payerUser.publicKey.toBase58());
    assert.equal(settlementState.payerUser.toBase58(), payerUser.publicKey.toBase58());
    assert.equal(payerLamportsAfter, payerLamportsBefore, "payer_user must not fund receipt rent or transaction fees");
    assert.isBelow(feePayerLamportsAfter, feePayerLamportsBefore, "fee_payer should fund receipt settlement rent and transaction fees");
    assert.equal(
      payerUsdcAfter.amount.toString(),
      (payerUsdcBefore.amount - BigInt(RECEIPT_CHARGE_USDC)).toString()
    );
    assert.equal(payerUsdcAfter.delegatedAmount.toString(), "0");
  });

  it("rejects receipt settlement when the supplied payment delegate is not approved", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance({
      amount: RECEIPT_CHARGE_USDC,
      delegate: feePayer.publicKey,
      mint: usdcMint,
      owner: payerUser,
      tokenAccount: payerUsdcAta,
    });

    await expectAnchorError(
      submitReceipt("delegate-wrong", {
        ...activeAlternateProviderOverrides(),
        feePayer: wrongPaymentDelegate,
        paymentDelegate: wrongPaymentDelegate,
        skipDelegateApproval: true,
      }),
      "InvalidPaymentDelegate"
    );
  });

  it("rejects receipt settlement when delegated allowance is below the receipt charge", async () => {
    await ensureSubmitFlowBootstrapped();

    await approvePayerAllowance({
      amount: RECEIPT_CHARGE_USDC - 1,
      delegate: feePayer.publicKey,
      mint: usdcMint,
      owner: payerUser,
      tokenAccount: payerUsdcAta,
    });

    await expectAnchorError(
      submitReceipt("delegate-low-allowance", {
        ...activeAlternateProviderOverrides(),
        skipDelegateApproval: true,
      }),
      "InsufficientDelegatedAllowance"
    );
  });

  it("submits with long off-chain metadata because only hashes go on chain", async () => {
    await ensureSubmitFlowBootstrapped();

    const receipt = await submitReceipt("phase1-long-metadata", {
      providerSigner: alternateProviderSignerPda,
      providerWallet: alternateProviderWallet.publicKey,
      providerAccount: alternateProviderAccountPda,
      providerRewardAccount: alternateProviderRewardPda,
      metadata: {
        proofId: `proof-${"x".repeat(512)}`,
        providerCode: `gateway/${"y".repeat(256)}`,
        model: `model-${"z".repeat(512)}`,
      },
    });

    assert.match(receipt.signature, /^[1-9A-HJ-NP-Za-km-z]{32,}$/);
  });

  it("rejects rogue usdc mint accounts after charge_mint removal", async () => {
    await ensureSubmitFlowBootstrapped();

    await expectAnchorError(
      submitReceipt("phase1-rogue-mint", {
        providerSigner: alternateProviderSignerPda,
        providerWallet: alternateProviderWallet.publicKey,
        providerAccount: alternateProviderAccountPda,
        providerRewardAccount: alternateProviderRewardPda,
        usdcMintOverride: rogueUsdcMint,
        skipDelegateApproval: true,
      }),
      "InvalidUsdcMint"
    );
  });

  it("rejects signer registrations without the gateway attester bit", async () => {
    await ensureSubmitFlowBootstrapped();

    const providerOnlySigner = Keypair.generate();
    const providerOnlySignerPda = PublicKey.findProgramAddressSync(
      [
        Buffer.from("provider_signer"),
        alternateProviderWallet.publicKey.toBuffer(),
        providerOnlySigner.publicKey.toBuffer(),
      ],
      attestation.programId
    )[0];

    await attestation.methods
      .upsertProviderSigner(
        providerOnlySigner.publicKey,
        alternateProviderWallet.publicKey,
        1 << 0,
        new BN(0),
        new BN(0)
      )
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        providerSigner: providerOnlySignerPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    await expectAnchorError(
      submitReceipt("phase1-provider-only-attester", {
        providerSigner: providerOnlySignerPda,
        signingKeypair: providerOnlySigner,
        providerWallet: alternateProviderWallet.publicKey,
        providerAccount: alternateProviderAccountPda,
        providerRewardAccount: alternateProviderRewardPda,
      }),
      "SignerAttesterTypeMismatch"
    );
  });

  async function fundFaucetClawVault(amount: number) {
    const source = await getAccount(provider.connection, payerClawAta);
    assert.isAtLeast(
      Number(source.amount),
      amount,
      "seeded CLAW account should hold existing supply for faucet funding"
    );

    await transferChecked(
      provider.connection,
      wallet.payer,
      payerClawAta,
      clawMint,
      faucetClawVaultPda,
      payerUser,
      BigInt(amount),
      6
    );
  }

  async function fundFaucetUsdcVault(amount: number) {
    await mintTo(
      provider.connection,
      wallet.payer,
      usdcMint,
      faucetUsdcVaultPda,
      wallet.payer,
      BigInt(amount)
    );
  }

  async function approvePayerAllowance({
    amount,
    delegate,
    mint,
    owner,
    tokenAccount,
  }: {
    amount: number;
    delegate: PublicKey;
    mint: PublicKey;
    owner: Keypair;
    tokenAccount: PublicKey;
  }) {
    await approveChecked(
      provider.connection,
      wallet.payer,
      mint,
      tokenAccount,
      delegate,
      owner,
      BigInt(amount),
      6
    );
  }

  function activeAlternateProviderOverrides() {
    return {
      providerSigner: alternateProviderSignerPda,
      providerWallet: alternateProviderWallet.publicKey,
      providerAccount: alternateProviderAccountPda,
      providerRewardAccount: alternateProviderRewardPda,
    };
  }

  async function submitReceipt(
    requestNonce: string,
    overrides?: {
      payerUser?: Keypair;
      payerUsdcToken?: PublicKey;
      userRewardAccount?: PublicKey;
      providerSigner?: PublicKey;
      providerWallet?: PublicKey;
      providerAccount?: PublicKey;
      providerRewardAccount?: PublicKey;
      usdcMintOverride?: PublicKey;
      metadata?: Partial<CompactReceiptMetadata>;
      promptTokens?: number;
      completionTokens?: number;
      chargeAtomic?: number;
      signingKeypair?: Keypair;
      feePayer?: Keypair;
      paymentDelegate?: Keypair;
      delegateAmount?: number;
      // Set when the browser wallet approval is issued explicitly by the test.
      skipDelegateApproval?: boolean;
      submitArgs?: ReturnType<typeof makeSubmitArgs>;
    }
  ) {
    const receiptPda = deriveReceiptPda(requestNonce);
    const settlementPda = deriveReceiptSettlementPda(receiptPda);
    const feePayerKeypair = overrides?.feePayer ?? feePayer;
    const paymentDelegateKeypair = overrides?.paymentDelegate ?? feePayerKeypair;
    const payerUserKeypair = overrides?.payerUser ?? payerUser;
    const payerUsdcToken = overrides?.payerUsdcToken ?? payerUsdcAta;
    const receiptUsdcMint = overrides?.usdcMintOverride ?? usdcMint;
    if (!overrides?.skipDelegateApproval) {
      await approvePayerAllowance({
        amount: overrides?.delegateAmount ?? overrides?.chargeAtomic ?? RECEIPT_CHARGE_USDC,
        delegate: paymentDelegateKeypair.publicKey,
        mint: receiptUsdcMint,
        owner: payerUserKeypair,
        tokenAccount: payerUsdcToken,
      });
    }
    const submit =
      overrides?.submitArgs ??
      makeSubmitArgs(requestNonce, {
        metadata: overrides?.metadata,
        payerUser: overrides?.payerUser?.publicKey,
        providerWallet: overrides?.providerWallet,
        usdcMint: overrides?.usdcMintOverride,
        promptTokens: overrides?.promptTokens,
        completionTokens: overrides?.completionTokens,
        chargeAtomic: overrides?.chargeAtomic,
      });
    const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: (overrides?.signingKeypair ?? providerSigner).secretKey,
      message: Uint8Array.from(submit.receiptHash),
    });
    const submitIx = await attestation.methods
      .submitReceipt(submit)
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        providerSigner: overrides?.providerSigner ?? providerSignerPda,
        receipt: receiptPda,
        payerUser: payerUserKeypair.publicKey,
        feePayer: feePayerKeypair.publicKey,
        paymentDelegate: paymentDelegateKeypair.publicKey,
        payerUsdcToken,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolProviderAccount: overrides?.providerAccount ?? providerAccountPda,
        masterpoolProviderRewardAccount:
          overrides?.providerRewardAccount ?? providerRewardPda,
        masterpoolUserRewardAccount:
          overrides?.userRewardAccount ?? userRewardPda,
        masterpoolReceiptSettlement: settlementPda,
        masterpoolTreasuryUsdcVault: treasuryUsdcVaultPda,
        masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
        usdcMint: receiptUsdcMint,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .instruction();

    const tx = new Transaction().add(ed25519Ix, submitIx);
    tx.feePayer = feePayerKeypair.publicKey;
    const submitSigners = feePayerKeypair.publicKey.equals(paymentDelegateKeypair.publicKey)
      ? [feePayerKeypair]
      : [feePayerKeypair, paymentDelegateKeypair];
    const signature = await provider.sendAndConfirm(tx, submitSigners);
    return { receiptPda, settlementPda, signature, submitArgs: submit };
  }

  async function ensureMasterpoolInitialized() {
    const masterpoolConfigExists = !!(await provider.connection.getAccountInfo(
      masterpoolConfigPda
    ));
    if (masterpoolConfigExists) {
      return;
    }

    await masterpool.methods
      .initializeMasterpool({
        exchangeRateClawPerUsdcE6: new BN(CLAW_UNIT),
        providerStakeUsdc: new BN(PROVIDER_STAKE_USDC),
        providerUsdcShareBps: 700,
        treasuryUsdcShareBps: 300,
        userClawShareBps: 300,
        providerClawShareBps: 700,
        lockDays: 180,
        providerSlashClawAmount: new BN(PROVIDER_SLASH_CLAW),
        challengerRewardBps: 700,
        burnBps: 300,
        challengeBondClawAmount: new BN(CHALLENGE_BOND_CLAW),
      })
      .accounts({
        config: masterpoolConfigPda,
        rewardVault: rewardVaultPda,
        challengeBondVault: challengeBondVaultPda,
        treasuryUsdcVault: treasuryUsdcVaultPda,
        providerStakeUsdcVault: providerStakeVaultPda,
        providerPendingUsdcVault: providerPendingVaultPda,
        clawMint,
        usdcMint,
        attestationProgram: attestation.programId,
        selfProgram: masterpool.programId,
        selfProgramData: masterpoolProgramData,
        poolAuthority: poolAuthorityPda,
        initializer: wallet.publicKey,
        admin: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();
  }

  async function ensureGenesisSupplyMinted() {
    await ensureMasterpoolInitialized();

    const config = await masterpool.account.globalConfig.fetch(masterpoolConfigPda);
    if (config.genesisMinted) {
      return;
    }

    await setAuthority(
      provider.connection,
      wallet.payer,
      clawMint,
      wallet.payer,
      AuthorityType.MintTokens,
      poolAuthorityPda
    );
    await setAuthority(
      provider.connection,
      wallet.payer,
      clawMint,
      wallet.payer,
      AuthorityType.FreezeAccount,
      poolAuthorityPda
    );

    await masterpool.methods
      .mintGenesisSupply()
      .accounts({
        config: masterpoolConfigPda,
        adminAuthority: wallet.publicKey,
        rewardVault: rewardVaultPda,
        clawMint,
        poolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();
  }

  async function ensureFaucetBootstrapped(options?: {
    enabled?: boolean;
    clawVaultBalance?: number;
    usdcVaultBalance?: number;
  }) {
    await ensureGenesisSupplyMinted();

    const faucetConfigExists = !!(await provider.connection.getAccountInfo(
      faucetConfigPda
    ));
    if (!faucetConfigExists) {
      await masterpool.methods
        .initializeFaucet()
        .accounts({
          config: masterpoolConfigPda,
          faucetConfig: faucetConfigPda,
          faucetGlobalState: faucetGlobalPda,
          faucetClawVault: faucetClawVaultPda,
          faucetUsdcVault: faucetUsdcVaultPda,
          clawMint,
          usdcMint,
          poolAuthority: poolAuthorityPda,
          adminAuthority: wallet.publicKey,
          payer: wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        } as any)
        .rpc();
    }

    await ensureFaucetVaultBalance(
      faucetClawVaultPda,
      options?.clawVaultBalance ?? 0,
      fundFaucetClawVault
    );
    await ensureFaucetVaultBalance(
      faucetUsdcVaultPda,
      options?.usdcVaultBalance ?? FAUCET_PER_WALLET_PER_DAY,
      fundFaucetUsdcVault
    );

    if (typeof options?.enabled === "boolean") {
      const faucetConfig = await (masterpool.account as any).faucetConfig.fetch(
        faucetConfigPda
      );
      if (faucetConfig.enabled !== options.enabled) {
        await masterpool.methods
          .setFaucetEnabled(options.enabled)
          .accounts({
            config: masterpoolConfigPda,
            faucetConfig: faucetConfigPda,
            adminAuthority: wallet.publicKey,
          } as any)
          .rpc();
      }
    }
  }

  async function ensureFaucetVaultBalance(
    vault: PublicKey,
    minimumBalance: number,
    fund: (amount: number) => Promise<void>
  ) {
    const account = await getAccount(provider.connection, vault);
    const currentBalance = Number(account.amount);
    if (currentBalance < minimumBalance) {
      await fund(minimumBalance - currentBalance);
    }
  }

  async function ensureSubmitFlowBootstrapped() {
    await ensureMasterpoolInitialized();

    const attestationConfigExists = !!(await provider.connection.getAccountInfo(
      attestationConfigPda
    ));
    if (!attestationConfigExists) {
      await attestation.methods
        .initializeConfig(
          attestationAuthority,
          pauseAuthority,
          challengeResolver,
          masterpool.programId,
          new BN(CHALLENGE_WINDOW_SECONDS),
          new BN(CHALLENGE_RESOLUTION_TIMEOUT_SECONDS)
        )
        .accounts({
          initializer: wallet.publicKey,
          config: attestationConfigPda,
          selfProgram: attestation.programId,
          selfProgramData: attestationProgramData,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc();
    }

    const providerSignerExists = !!(await provider.connection.getAccountInfo(
      providerSignerPda
    ));
    if (!providerSignerExists) {
      await attestation.methods
        .upsertProviderSigner(
          providerSigner.publicKey,
          providerWallet.publicKey,
          1 << 1,
          new BN(0),
          new BN(0)
        )
        .accounts({
          authority: wallet.publicKey,
          config: attestationConfigPda,
          providerSigner: providerSignerPda,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc();
    }

    const alternateProviderSignerExists = !!(await provider.connection.getAccountInfo(
      alternateProviderSignerPda
    ));
    if (!alternateProviderSignerExists) {
      await attestation.methods
        .upsertProviderSigner(
          providerSigner.publicKey,
          alternateProviderWallet.publicKey,
          1 << 1,
          new BN(0),
          new BN(0)
        )
        .accounts({
          authority: wallet.publicKey,
          config: attestationConfigPda,
          providerSigner: alternateProviderSignerPda,
          systemProgram: SystemProgram.programId,
        } as any)
        .rpc();
    }

    const providerAccountExists = !!(await provider.connection.getAccountInfo(
      providerAccountPda
    ));
    if (!providerAccountExists) {
      await masterpool.methods
        .registerProvider()
        .accounts({
          config: masterpoolConfigPda,
          providerAccount: providerAccountPda,
          providerRewardAccount: providerRewardPda,
          providerWallet: providerWallet.publicKey,
          providerStakeUsdcVault: providerStakeVaultPda,
          providerUsdcToken: providerUsdcAta,
          usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([providerWallet])
        .rpc();
    }

    const alternateProviderAccountExists = !!(await provider.connection.getAccountInfo(
      alternateProviderAccountPda
    ));
    if (!alternateProviderAccountExists) {
      await masterpool.methods
        .registerProvider()
        .accounts({
          config: masterpoolConfigPda,
          providerAccount: alternateProviderAccountPda,
          providerRewardAccount: alternateProviderRewardPda,
          providerWallet: alternateProviderWallet.publicKey,
          providerStakeUsdcVault: providerStakeVaultPda,
          providerUsdcToken: alternateProviderUsdcAta,
          usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([alternateProviderWallet])
        .rpc();
    }
  }

  async function finalizeReceipt(receiptPda: PublicKey, settlementPda: PublicKey) {
    await attestation.methods
      .finalizeReceipt()
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        receipt: receiptPda,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolReceiptSettlement: settlementPda,
        masterpoolProviderAccount: providerAccountPda,
        masterpoolProviderRewardAccount: providerRewardPda,
        masterpoolUserRewardAccount: userRewardPda,
        masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
        masterpoolProviderDestinationUsdc: providerUsdcAta,
        usdcMint,
        masterpoolPoolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();
  }

  async function openChallenge(receipt: { receiptPda: PublicKey; settlementPda: PublicKey }) {
    const challengePda = deriveChallengePda(receipt.receiptPda);
    const challengeBondRecordPda = deriveChallengeBondRecordPda(challengePda);
    await attestation.methods
      .openChallenge(4, Array.from(fillBytes(32, 7)))
      .accounts({
        challenger: payerUser.publicKey,
        config: attestationConfigPda,
        receipt: receipt.receiptPda,
        challenge: challengePda,
        challengerClawToken: payerClawAta,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolReceiptSettlement: receipt.settlementPda,
        masterpoolProviderAccount: providerAccountPda,
        masterpoolChallengeBondRecord: challengeBondRecordPda,
        masterpoolChallengeBondVault: challengeBondVaultPda,
        clawMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([payerUser])
      .rpc();
    return { challengePda, challengeBondRecordPda };
  }

  async function resolveChallenge(
    receipt: { receiptPda: PublicKey; settlementPda: PublicKey },
    challenge: { challengePda: PublicKey; challengeBondRecordPda: PublicKey },
    resolutionCode: number
  ) {
    await attestation.methods
      .resolveChallenge(resolutionCode)
      .accounts({
        challengeResolver: wallet.publicKey,
        config: attestationConfigPda,
        receipt: receipt.receiptPda,
        challenge: challenge.challengePda,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolReceiptSettlement: receipt.settlementPda,
        masterpoolChallengeBondRecord: challenge.challengeBondRecordPda,
        masterpoolProviderAccount: providerAccountPda,
        masterpoolProviderRewardAccount: providerRewardPda,
        masterpoolUserRewardAccount: userRewardPda,
        masterpoolChallengeBondVault: challengeBondVaultPda,
        masterpoolRewardVault: rewardVaultPda,
        masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
        challengerClawToken: payerClawAta,
        payerUsdcToken: payerUsdcAta,
        clawMint,
        usdcMint,
        masterpoolPoolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();
  }

  async function timeoutRejectChallenge(
    receipt: { receiptPda: PublicKey; settlementPda: PublicKey },
    challenge: { challengePda: PublicKey; challengeBondRecordPda: PublicKey }
  ) {
    await attestation.methods
      .timeoutRejectChallenge()
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        receipt: receipt.receiptPda,
        challenge: challenge.challengePda,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolReceiptSettlement: receipt.settlementPda,
        masterpoolChallengeBondRecord: challenge.challengeBondRecordPda,
        masterpoolProviderAccount: providerAccountPda,
        masterpoolProviderRewardAccount: providerRewardPda,
        masterpoolUserRewardAccount: userRewardPda,
        masterpoolChallengeBondVault: challengeBondVaultPda,
        masterpoolRewardVault: rewardVaultPda,
        masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
        challengerClawToken: payerClawAta,
        payerUsdcToken: payerUsdcAta,
        clawMint,
        usdcMint,
        masterpoolPoolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();
  }

  async function waitForReceiptFinalizable(receiptPda: PublicKey) {
    const receipt = await attestation.account.receipt.fetch(receiptPda);
    await waitUntilAfter(decodeI64(receipt.challengeDeadline));
  }

  async function waitForChallengeTimeout(challengePda: PublicKey) {
    const [challenge, config] = await Promise.all([
      attestation.account.challenge.fetch(challengePda),
      attestation.account.config.fetch(attestationConfigPda),
    ]);

    await waitUntilAfter(
      decodeI64(challenge.openedAt) +
        decodeI64(config.challengeResolutionTimeoutSeconds)
    );
  }

  async function materializeRewardRelease(
    rewardAccount: PublicKey,
    settlementPda: PublicKey,
    target: number,
    amount: number
  ) {
    await masterpool.methods
      .materializeRewardRelease(target, new BN(amount))
      .accounts({
        config: masterpoolConfigPda,
        adminAuthority: wallet.publicKey,
        rewardAccount,
        receiptSettlement: settlementPda,
      } as any)
      .rpc();
  }

  async function releaseAndClaimUser(
    settlementPda: PublicKey,
    amount: number
  ) {
    await materializeRewardRelease(userRewardPda, settlementPda, 0, amount);
    await masterpool.methods
      .claimReleasedClaw()
      .accounts({
        config: masterpoolConfigPda,
        rewardAccount: userRewardPda,
        claimant: payerUser.publicKey,
        rewardVault: rewardVaultPda,
        claimantClawToken: payerClawAta,
        clawMint,
        poolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([payerUser])
      .rpc();
  }

  async function claimProviderReleasedClaw() {
    await masterpool.methods
      .claimReleasedClaw()
      .accounts({
        config: masterpoolConfigPda,
        rewardAccount: providerRewardPda,
        claimant: providerWallet.publicKey,
        rewardVault: rewardVaultPda,
        claimantClawToken: providerClawAta,
        clawMint,
        poolAuthority: poolAuthorityPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([providerWallet])
      .rpc();
  }

  function makeSubmitArgs(
    requestNonce: string,
    overrides?: {
      metadata?: Partial<CompactReceiptMetadata>;
      payerUser?: PublicKey;
      providerWallet?: PublicKey;
      usdcMint?: PublicKey;
      promptTokens?: number;
      completionTokens?: number;
      chargeAtomic?: number;
    }
  ) {
    const promptTokens = overrides?.promptTokens ?? 123;
    const completionTokens = overrides?.completionTokens ?? 456;
    const chargeAtomic = overrides?.chargeAtomic ?? RECEIPT_CHARGE_USDC;
    const metadata = buildCompactReceiptMetadata({
      proofId: "r",
      providerCode,
      model: "m",
      ...overrides?.metadata,
    });
    const providerWalletKey = overrides?.providerWallet ?? providerWallet.publicKey;
    const payerUserKey = overrides?.payerUser ?? payerUser.publicKey;
    const usdcMintKey = overrides?.usdcMint ?? usdcMint;
    return buildCompactSubmitArgs(
      {
        requestNonce,
        metadata,
        providerWallet: providerWalletKey,
        payerUser: payerUserKey,
        usdcMint: usdcMintKey,
        promptTokens,
        completionTokens,
        chargeAtomic,
      },
      (value) => new BN(value.toString())
    );
  }

  function deriveReceiptPda(requestNonce: string): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("receipt"), hashRequestNonce(requestNonce)],
      attestation.programId
    )[0];
  }

  function deriveChallengePda(receiptPda: PublicKey): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("challenge"), receiptPda.toBuffer()],
      attestation.programId
    )[0];
  }

  function deriveReceiptSettlementPda(receiptPda: PublicKey): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("receipt_settlement"), receiptPda.toBuffer()],
      masterpool.programId
    )[0];
  }

  function deriveChallengeBondRecordPda(challengePda: PublicKey): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("challenge_bond_record"), challengePda.toBuffer()],
      masterpool.programId
    )[0];
  }
});

async function airdrop(pubkey: PublicKey): Promise<void> {
  const provider = anchor.AnchorProvider.env();
  const signature = await provider.connection.requestAirdrop(
    pubkey,
    2 * anchor.web3.LAMPORTS_PER_SOL
  );
  const latest = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction(
    {
      signature,
      blockhash: latest.blockhash,
      lastValidBlockHeight: latest.lastValidBlockHeight,
    },
    "confirmed"
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitUntilAfter(targetUnixSeconds: number): Promise<void> {
  for (;;) {
    const provider = anchor.AnchorProvider.env();
    const slot = await provider.connection.getSlot("processed");
    const blockTime = await provider.connection.getBlockTime(slot);
    const now = blockTime ?? Math.floor(Date.now() / 1000);
    if (now > targetUnixSeconds) {
      return;
    }
    await sleep(500);
  }
}

function decodeI64(value: anchor.BN | number): number {
  return typeof value === "number" ? value : value.toNumber();
}

function decodeU64(value: anchor.BN | number): number {
  return typeof value === "number" ? value : value.toNumber();
}

async function currentUnixTime(): Promise<number> {
  const provider = anchor.AnchorProvider.env();
  const slot = await provider.connection.getSlot("processed");
  const blockTime = await provider.connection.getBlockTime(slot);
  return blockTime ?? Math.floor(Date.now() / 1000);
}

function computeLinearReleasableAmount(
  totalLocked: number,
  releasedSoFar: number,
  lockStart: number,
  now: number,
  lockDays: number
): number {
  const lockSeconds = lockDays * 86_400;
  assert.isAbove(lockSeconds, 0);

  const elapsed = Math.max(0, Math.min(now - lockStart, lockSeconds));
  const vested = Math.floor((totalLocked * elapsed) / lockSeconds);
  return vested - releasedSoFar;
}

function deriveProgramDataAddress(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [programId.toBuffer()],
    UPGRADEABLE_LOADER_PROGRAM_ID
  )[0];
}

function fillBytes(length: number, value: number): Uint8Array {
  return Uint8Array.from({ length }, () => value);
}

async function expectAnchorError(
  promise: Promise<unknown>,
  expected: string
): Promise<void> {
  try {
    await promise;
    assert.fail(`expected error containing ${expected}`);
  } catch (error: any) {
    if (String(error).includes(`expected error containing ${expected}`)) {
      throw error;
    }
    assert.include(String(error), expected);
  }
}
