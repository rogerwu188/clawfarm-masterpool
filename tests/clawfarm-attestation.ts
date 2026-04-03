import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
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
import { ClawfarmAttestation } from "../target/types/clawfarm_attestation";

describe("clawfarm-attestation", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program =
    anchor.workspace.ClawfarmAttestation as Program<ClawfarmAttestation>;

  const wallet = provider.wallet as anchor.Wallet;
  const authority = wallet.publicKey;
  const pauseAuthority = wallet.publicKey;
  const challengeResolver = wallet.publicKey;
  const providerCode = "unipass";
  const keyId = "phase1-key-1";
  const attesterType = 1;
  const attesterTypeMask = 1 << attesterType;
  const challengeType = 4;
  const resolutionRejected = 2;

  const providerSigner = Keypair.generate();
  const challenger = Keypair.generate();
  const outsider = Keypair.generate();

  let configPda: PublicKey;
  let providerSignerPda: PublicKey;

  before(async () => {
    [configPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );
    [providerSignerPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("provider_signer"),
        sha256(Buffer.from(providerCode)),
        providerSigner.publicKey.toBuffer(),
      ],
      program.programId
    );

    await airdrop(challenger.publicKey);
    await airdrop(outsider.publicKey);
  });

  it("initializes config", async () => {
    await program.methods
      .initializeConfig(
        authority,
        pauseAuthority,
        challengeResolver,
        new BN(30),
        new BN(30)
      )
      .accounts({
        payer: wallet.publicKey,
        config: configPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    const config = await program.account.config.fetch(configPda);
    assert.ok(config.authority.equals(authority));
    assert.ok(config.pauseAuthority.equals(pauseAuthority));
    assert.ok(config.challengeResolver.equals(challengeResolver));
    assert.equal(config.receiptCount.toNumber(), 0);
    assert.equal(config.challengeCount.toNumber(), 0);
    assert.equal(config.isPaused, false);
  });

  it("upserts provider signer", async () => {
    await program.methods
      .upsertProviderSigner(
        providerCode,
        providerSigner.publicKey,
        keyId,
        attesterTypeMask,
        new BN(0),
        new BN(0),
        Array.from(new Uint8Array(32))
      )
      .accounts({
        authority,
        config: configPda,
        providerSigner: providerSignerPda,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    const signerAccount = await program.account.providerSigner.fetch(
      providerSignerPda
    );
    assert.equal(signerAccount.providerCode, providerCode);
    assert.ok(signerAccount.signer.equals(providerSigner.publicKey));
    assert.equal(signerAccount.status, 1);
    assert.equal(signerAccount.attesterTypeMask, attesterTypeMask);
  });

  it("rejects submit_receipt without ed25519 pre-instruction", async () => {
    const requestNonce = "cfn_missing_sig_001";
    const submit = makeSubmitArgs(requestNonce);
    const receiptPda = deriveReceiptPda(submit.requestNonce);
    const ix = await program.methods
      .submitReceipt(submit)
      .accounts({
        payer: wallet.publicKey,
        config: configPda,
        providerSigner: providerSignerPda,
        receipt: receiptPda,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      } as any)
      .instruction();

    const tx = new Transaction().add(ix);
    await expectAnchorError(
      provider.sendAndConfirm(tx),
      "MissingEd25519Instruction"
    );
  });

  it("submits a verified receipt and records it", async () => {
    const requestNonce = "cfn_submit_ok_001";
    const submit = makeSubmitArgs(requestNonce, {
      providerRequestId: "req_123",
      issuedAt: 1_711_950_000,
      expiresAt: 1_711_953_600,
      httpStatus: 200,
      latencyMs: 1840,
    });
    const receiptPda = deriveReceiptPda(submit.requestNonce);

    await sendSubmitReceipt(submit);

    const receipt = await program.account.receipt.fetch(receiptPda);
    const config = await program.account.config.fetch(configPda);
    assert.equal(receipt.requestNonce, requestNonce);
    assert.equal(receipt.proofId, "cap_test_001");
    assert.equal(receipt.provider, providerCode);
    assert.ok(receipt.signer.equals(providerSigner.publicKey));
    assert.deepEqual(
      receipt.proofUrlHash,
      Array.from(
        sha256(
          Buffer.from(
            "https://provider.example.com/api/public/v1/proofs/cap_test_001"
          )
        )
      )
    );
    assert.equal(receipt.status, 0);
    assert.equal(config.receiptCount.toNumber(), 1);
  });

  it("opens a challenge, blocks unauthorized response, and resolves it", async () => {
    const requestNonce = "cfn_submit_ok_001";
    const receiptPda = deriveReceiptPda(requestNonce);
    const challengePda = deriveChallengePda(
      receiptPda,
      challengeType,
      challenger.publicKey
    );
    const evidenceHash = Array.from(fillBytes(32, 7));
    const responseHash = Array.from(fillBytes(32, 9));

    await program.methods
      .openChallenge(requestNonce, challengeType, evidenceHash)
      .accounts({
        challenger: challenger.publicKey,
        config: configPda,
        receipt: receiptPda,
        challenge: challengePda,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([challenger])
      .rpc();

    let receipt = await program.account.receipt.fetch(receiptPda);
    let challenge = await program.account.challenge.fetch(challengePda);
    assert.equal(receipt.status, 1);
    assert.equal(challenge.status, 0);
    assert.ok(challenge.challenger.equals(challenger.publicKey));

    await expectAnchorError(
      program.methods
        .respondChallenge(requestNonce, challengeType, challenger.publicKey, responseHash)
        .accounts({
          responder: outsider.publicKey,
          config: configPda,
          receipt: receiptPda,
          challenge: challengePda,
        } as any)
        .signers([outsider])
        .rpc(),
      "ChallengeResponderUnauthorized"
    );

    await program.methods
      .respondChallenge(requestNonce, challengeType, challenger.publicKey, responseHash)
      .accounts({
        responder: authority,
        config: configPda,
        receipt: receiptPda,
        challenge: challengePda,
      } as any)
      .rpc();

    await program.methods
      .resolveChallenge(
        requestNonce,
        challengeType,
        challenger.publicKey,
        resolutionRejected
      )
      .accounts({
        challengeResolver,
        config: configPda,
        receipt: receiptPda,
        challenge: challengePda,
      } as any)
      .rpc();

    receipt = await program.account.receipt.fetch(receiptPda);
    challenge = await program.account.challenge.fetch(challengePda);
    const config = await program.account.config.fetch(configPda);

    assert.equal(receipt.status, 0);
    assert.equal(challenge.status, 3);
    assert.equal(challenge.resolutionCode, resolutionRejected);
    assert.equal(config.challengeCount.toNumber(), 1);
  });

  function makeSubmitArgs(
    requestNonce: string,
    overrides?: {
      providerRequestId?: string;
      issuedAt?: number;
      expiresAt?: number;
      httpStatus?: number;
      latencyMs?: number;
    }
  ) {
    const logicalPayload = {
      version: 1,
      proof_mode: "sig_log",
      proof_id: "cap_test_001",
      request_nonce: requestNonce,
      provider: providerCode,
      attester_type: "gateway",
      model: "openai/gpt-4.1",
      usage_basis: "provider_reported",
      prompt_tokens: new BN(123),
      completion_tokens: new BN(456),
      total_tokens: new BN(579),
      charge_atomic: "1250000",
      charge_mint: wallet.publicKey.toBase58(),
      provider_request_id: overrides?.providerRequestId,
      issued_at: overrides?.issuedAt,
      expires_at: overrides?.expiresAt,
      http_status: overrides?.httpStatus,
      latency_ms: overrides?.latencyMs,
    };

    const receiptHash = sha256(encodeCanonicalPayload(logicalPayload));
    const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: providerSigner.secretKey,
      message: receiptHash,
    });
    const signature = extractSignature(ed25519Ix.data);

    return {
      version: logicalPayload.version,
      proofMode: 0,
      proofId: logicalPayload.proof_id,
      requestNonce: logicalPayload.request_nonce,
      provider: logicalPayload.provider,
      attesterType: attesterType,
      model: logicalPayload.model,
      usageBasis: 0,
      promptTokens: logicalPayload.prompt_tokens,
      completionTokens: logicalPayload.completion_tokens,
      totalTokens: logicalPayload.total_tokens,
      chargeAtomic: new BN(logicalPayload.charge_atomic),
      chargeMint: wallet.publicKey,
      providerRequestId: logicalPayload.provider_request_id ?? null,
      issuedAt:
        logicalPayload.issued_at !== undefined
          ? new BN(logicalPayload.issued_at)
          : null,
      expiresAt:
        logicalPayload.expires_at !== undefined
          ? new BN(logicalPayload.expires_at)
          : null,
      httpStatus: logicalPayload.http_status ?? null,
      latencyMs:
        logicalPayload.latency_ms !== undefined
          ? new BN(logicalPayload.latency_ms)
          : null,
      proofUrl:
        "https://provider.example.com/api/public/v1/proofs/cap_test_001",
      receiptHash: Array.from(receiptHash),
      signer: providerSigner.publicKey,
      signature: Array.from(signature),
    };
  }

  async function sendSubmitReceipt(
    submit: ReturnType<typeof makeSubmitArgs>
  ): Promise<void> {
    const receiptPda = deriveReceiptPda(submit.requestNonce);
    const ed25519Ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: providerSigner.secretKey,
      message: Uint8Array.from(submit.receiptHash),
    });
    const submitIx = await program.methods
      .submitReceipt(submit)
      .accounts({
        payer: wallet.publicKey,
        config: configPda,
        providerSigner: providerSignerPda,
        receipt: receiptPda,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      } as any)
      .instruction();

    const tx = new Transaction().add(ed25519Ix, submitIx);
    await provider.sendAndConfirm(tx);
  }

  async function airdrop(pubkey: PublicKey): Promise<void> {
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

  function deriveReceiptPda(requestNonce: string): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("receipt"), sha256(Buffer.from(requestNonce))],
      program.programId
    )[0];
  }

  function deriveChallengePda(
    receipt: PublicKey,
    challengeTypeValue: number,
    challengerKey: PublicKey
  ): PublicKey {
    return PublicKey.findProgramAddressSync(
      [
        Buffer.from("challenge"),
        receipt.toBuffer(),
        Buffer.from([challengeTypeValue]),
        challengerKey.toBuffer(),
      ],
      program.programId
    )[0];
  }
});

function encodeCanonicalPayload(payload: {
  version: number;
  proof_mode: string;
  proof_id: string;
  request_nonce: string;
  provider: string;
  attester_type: string;
  model: string;
  usage_basis: string;
  prompt_tokens: BN;
  completion_tokens: BN;
  total_tokens: BN;
  charge_atomic: string;
  charge_mint: string;
  provider_request_id?: string;
  issued_at?: number;
  expires_at?: number;
  http_status?: number;
  latency_ms?: number;
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

  if (payload.provider_request_id !== undefined) {
    entries.push([
      "provider_request_id",
      encodeText(payload.provider_request_id),
    ]);
  }
  if (payload.issued_at !== undefined) {
    entries.push(["issued_at", encodeSigned(payload.issued_at)]);
  }
  if (payload.expires_at !== undefined) {
    entries.push(["expires_at", encodeSigned(payload.expires_at)]);
  }
  if (payload.http_status !== undefined) {
    entries.push(["http_status", encodeUnsigned(payload.http_status)]);
  }
  if (payload.latency_ms !== undefined) {
    entries.push(["latency_ms", encodeUnsigned(payload.latency_ms)]);
  }

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

function encodeSigned(value: number): Buffer {
  if (value >= 0) {
    return encodeUnsigned(value);
  }
  return encodeMajorLen(1, -1 - value);
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

function extractSignature(data: Buffer | Uint8Array): Buffer {
  const bytes = Buffer.from(data);
  const signatureOffset = bytes.readUInt16LE(2);
  return bytes.subarray(signatureOffset, signatureOffset + 64);
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
    const message = String(error);
    assert.include(message, expected);
  }
}
