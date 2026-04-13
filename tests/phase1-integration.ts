import * as anchor from "@coral-xyz/anchor";
import { assert } from "chai";
import crypto from "crypto";
import {
  Ed25519Program,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

const USDC_UNIT = 1_000_000;
const CLAW_UNIT = 1_000_000;
const PROVIDER_STAKE_USDC = 100 * USDC_UNIT;
const RECEIPT_CHARGE_USDC = 10 * USDC_UNIT;
const CHALLENGE_BOND_CLAW = 2 * CLAW_UNIT;
const PROVIDER_SLASH_CLAW = 30 * CLAW_UNIT;
const USER_REWARD_PER_RECEIPT = 3 * CLAW_UNIT;
const PROVIDER_REWARD_PER_RECEIPT = 7 * CLAW_UNIT;

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
  const providerCode = "unipass";
  const providerSigner = Keypair.generate();
  const providerWallet = Keypair.generate();
  const payerUser = Keypair.generate();

  let clawMint: PublicKey;
  let usdcMint: PublicKey;
  let masterpoolConfigPda: PublicKey;
  let rewardVaultPda: PublicKey;
  let challengeBondVaultPda: PublicKey;
  let treasuryUsdcVaultPda: PublicKey;
  let providerStakeVaultPda: PublicKey;
  let providerPendingVaultPda: PublicKey;
  let poolAuthorityPda: PublicKey;
  let attestationConfigPda: PublicKey;
  let providerSignerPda: PublicKey;
  let providerAccountPda: PublicKey;
  let providerRewardPda: PublicKey;
  let userRewardPda: PublicKey;

  let providerUsdcAta: PublicKey;
  let payerUsdcAta: PublicKey;
  let payerClawAta: PublicKey;
  let providerClawAta: PublicKey;

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
    [attestationConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      attestation.programId
    );
    [providerSignerPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("provider_signer"),
        sha256(Buffer.from(providerCode)),
        providerSigner.publicKey.toBuffer(),
      ],
      attestation.programId
    );
    [providerAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), providerWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [providerRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider_reward"), providerWallet.publicKey.toBuffer()],
      masterpool.programId
    );
    [userRewardPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_reward"), payerUser.publicKey.toBuffer()],
      masterpool.programId
    );

    await airdrop(providerWallet.publicKey);
    await airdrop(payerUser.publicKey);

    clawMint = await createMint(
      provider.connection,
      wallet.payer,
      poolAuthorityPda,
      poolAuthorityPda,
      6
    );
    usdcMint = await createMint(
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
    payerUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        wallet.payer,
        usdcMint,
        payerUser.publicKey
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
      payerUsdcAta,
      wallet.publicKey,
      1_000 * USDC_UNIT
    );
  });

  it("executes the Phase 1 receipt, challenge, and reward lifecycle", async () => {
    await masterpool.methods
      .initializeMasterpool({
        exchangeRateClawPerUsdcE6: new BN(CLAW_UNIT),
        providerStakeUsdc: new BN(PROVIDER_STAKE_USDC),
        providerUsdcShareBps: 300,
        treasuryUsdcShareBps: 700,
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
        poolAuthority: poolAuthorityPda,
        admin: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

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
        new BN(1)
      )
      .accounts({
        payer: wallet.publicKey,
        config: attestationConfigPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    await attestation.methods
      .upsertProviderSigner(
        providerCode,
        providerSigner.publicKey,
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

    const unauthorizedReceipt = Keypair.generate().publicKey;
    const unauthorizedSettlement = deriveReceiptSettlementPda(unauthorizedReceipt);
    await expectAnchorError(
      masterpool.methods
        .recordMiningFromReceipt({
          totalUsdcPaid: new BN(RECEIPT_CHARGE_USDC),
          chargeMint: usdcMint,
        })
        .accounts({
          config: masterpoolConfigPda,
          attestationConfig: attestationConfigPda,
          payerUser: payerUser.publicKey,
          payerUsdcToken: payerUsdcAta,
          providerWallet: providerWallet.publicKey,
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
        .signers([payerUser])
        .rpc(),
      "Signature verification failed"
    );

    const receipt1 = await submitReceipt("phase1_receipt_1");
    await sleep(2_500);
    await finalizeReceipt(receipt1.receiptPda, receipt1.settlementPda);

    await releaseAndClaimUser(USER_REWARD_PER_RECEIPT);
    assert.equal(Number((await getAccount(provider.connection, payerClawAta)).amount), 3_000_000);

    const receipt2 = await submitReceipt("phase1_receipt_2");
    const challenge2 = await openChallenge(receipt2);
    assert.equal(Number((await getAccount(provider.connection, payerClawAta)).amount), 1_000_000);
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
    await releaseAndClaimUser(USER_REWARD_PER_RECEIPT);
    assert.equal(Number((await getAccount(provider.connection, payerClawAta)).amount), 4_000_000);

    const receipt3 = await submitReceipt("phase1_receipt_3");
    const challenge3 = await openChallenge(receipt3);
    await resolveChallenge(receipt3, challenge3, 1);
    assert.equal(Number((await getAccount(provider.connection, payerClawAta)).amount), 25_000_000);
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

    const receipt4 = await submitReceipt("phase1_receipt_4");
    await expectAnchorError(
      submitReceipt("phase1_receipt_4"),
      "already in use"
    );
    await sleep(2_500);
    await finalizeReceipt(receipt4.receiptPda, receipt4.settlementPda);

    const receipt5 = await submitReceipt("phase1_receipt_5");
    await sleep(2_500);
    await finalizeReceipt(receipt5.receiptPda, receipt5.settlementPda);

    await masterpool.methods
      .materializeRewardRelease(new BN(26 * CLAW_UNIT))
      .accounts({
        config: masterpoolConfigPda,
        adminAuthority: wallet.publicKey,
        rewardAccount: providerRewardPda,
      } as any)
      .rpc();
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

    assert.equal(
      Number((await getAccount(provider.connection, providerClawAta)).amount),
      26_000_000
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

    const providerState = await masterpool.account.providerAccount.fetch(providerAccountPda);
    assert.equal(providerState.status, 1);
    assert.equal(providerState.pendingProviderUsdc.toNumber(), 0);
    assert.equal(providerState.unsettledReceiptCount.toNumber(), 0);
    assert.equal(providerState.unresolvedChallengeCount.toNumber(), 0);
    assert.equal(providerState.clawNetPosition.toNumber(), 5_000_000);

    const settlement3 = await masterpool.account.receiptSettlement.fetch(receipt3.settlementPda);
    assert.equal(settlement3.status, 2);

    const challengeRecord3 = await masterpool.account.challengeBondRecord.fetch(
      challenge3.challengeBondRecordPda
    );
    assert.equal(challengeRecord3.status, 1);
    assert.equal(
      Number((await getAccount(provider.connection, payerUsdcAta)).amount),
      953_000_000
    );
  });

  async function submitReceipt(requestNonce: string) {
    const receiptPda = deriveReceiptPda(requestNonce);
    const settlementPda = deriveReceiptSettlementPda(receiptPda);
    const submit = makeSubmitArgs(requestNonce);
    const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: providerSigner.secretKey,
      message: Uint8Array.from(submit.receiptHash),
    });
    const submitIx = await attestation.methods
      .submitReceipt(submit)
      .accounts({
        authority: wallet.publicKey,
        config: attestationConfigPda,
        providerSigner: providerSignerPda,
        receipt: receiptPda,
        payerUser: payerUser.publicKey,
        payerUsdcToken: payerUsdcAta,
        providerWallet: providerWallet.publicKey,
        masterpoolConfig: masterpoolConfigPda,
        masterpoolProgram: masterpool.programId,
        masterpoolProviderAccount: providerAccountPda,
        masterpoolProviderRewardAccount: providerRewardPda,
        masterpoolUserRewardAccount: userRewardPda,
        masterpoolReceiptSettlement: settlementPda,
        masterpoolTreasuryUsdcVault: treasuryUsdcVaultPda,
        masterpoolProviderPendingUsdcVault: providerPendingVaultPda,
        usdcMint,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .instruction();

    const tx = new Transaction().add(ed25519Ix, submitIx);
    await provider.sendAndConfirm(tx, [payerUser]);
    return { receiptPda, settlementPda };
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

  async function releaseAndClaimUser(amount: number) {
    await masterpool.methods
      .materializeRewardRelease(new BN(amount))
      .accounts({
        config: masterpoolConfigPda,
        adminAuthority: wallet.publicKey,
        rewardAccount: userRewardPda,
      } as any)
      .rpc();
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

  function makeSubmitArgs(requestNonce: string) {
    const logicalPayload = {
      version: 1,
      proof_mode: "sig_log",
      proof_id: "phase1_receipt",
      request_nonce: requestNonce,
      provider: providerCode,
      attester_type: "gateway",
      model: "openai/gpt-4.1",
      usage_basis: "provider_reported",
      prompt_tokens: new BN(123),
      completion_tokens: new BN(456),
      total_tokens: new BN(579),
      charge_atomic: String(RECEIPT_CHARGE_USDC),
      charge_mint: usdcMint.toBase58(),
    };

    const receiptHash = sha256(encodeCanonicalPayload(logicalPayload));
    return {
      version: logicalPayload.version,
      proofMode: 0,
      proofId: logicalPayload.proof_id,
      requestNonce: logicalPayload.request_nonce,
      provider: logicalPayload.provider,
      attesterType: 1,
      model: logicalPayload.model,
      usageBasis: 0,
      promptTokens: logicalPayload.prompt_tokens,
      completionTokens: logicalPayload.completion_tokens,
      totalTokens: logicalPayload.total_tokens,
      chargeAtomic: new BN(RECEIPT_CHARGE_USDC),
      chargeMint: usdcMint,
      providerRequestId: null,
      issuedAt: null,
      expiresAt: null,
      httpStatus: null,
      latencyMs: null,
      receiptHash: Array.from(receiptHash),
      signer: providerSigner.publicKey,
    };
  }

  function deriveReceiptPda(requestNonce: string): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("receipt"), sha256(Buffer.from(requestNonce))],
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

function encodeCanonicalPayload(payload: {
  version: number;
  proof_mode: string;
  proof_id: string;
  request_nonce: string;
  provider: string;
  attester_type: string;
  model: string;
  usage_basis: string;
  prompt_tokens: anchor.BN;
  completion_tokens: anchor.BN;
  total_tokens: anchor.BN;
  charge_atomic: string;
  charge_mint: string;
}): Buffer {
  const entries: Array<[string, Buffer]> = [
    ["version", encodeUnsigned(payload.version)],
    ["proof_mode", encodeText(payload.proof_mode)],
    ["proof_id", encodeText(payload.proof_id)],
    ["request_nonce", encodeText(payload.request_nonce)],
    ["provider", encodeText(payload.provider)],
    ["attester_type", encodeText(payload.attester_type)],
    ["model", encodeText(payload.model)],
    ["usage_basis", encodeText(payload.usage_basis)],
    ["prompt_tokens", encodeUnsigned(payload.prompt_tokens.toNumber())],
    ["completion_tokens", encodeUnsigned(payload.completion_tokens.toNumber())],
    ["total_tokens", encodeUnsigned(payload.total_tokens.toNumber())],
    ["charge_atomic", encodeText(payload.charge_atomic)],
    ["charge_mint", encodeText(payload.charge_mint)],
  ];

  entries.sort(([left], [right]) =>
    Buffer.compare(encodeText(left), encodeText(right))
  );

  const out: Buffer[] = [encodeMajorLen(5, entries.length)];
  for (const [key, value] of entries) {
    out.push(encodeText(key), value);
  }
  return Buffer.concat(out);
}

function encodeText(value: string): Buffer {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([encodeMajorLen(3, bytes.length), bytes]);
}

function encodeUnsigned(value: number): Buffer {
  return encodeMajorLen(0, value);
}

function encodeMajorLen(major: number, value: number): Buffer {
  if (value <= 23) {
    return Buffer.from([(major << 5) | value]);
  }
  if (value <= 0xff) {
    return Buffer.from([(major << 5) | 24, value]);
  }
  if (value <= 0xffff) {
    const buf = Buffer.alloc(3);
    buf[0] = (major << 5) | 25;
    buf.writeUInt16BE(value, 1);
    return buf;
  }
  if (value <= 0xffffffff) {
    const buf = Buffer.alloc(5);
    buf[0] = (major << 5) | 26;
    buf.writeUInt32BE(value, 1);
    return buf;
  }
  const buf = Buffer.alloc(9);
  buf[0] = (major << 5) | 27;
  buf.writeBigUInt64BE(BigInt(value), 1);
  return buf;
}

function sha256(data: Buffer): Buffer {
  return crypto.createHash("sha256").update(data).digest();
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
    assert.include(String(error), expected);
  }
}
