import { expect } from "chai";
import { Connection, PublicKey } from "@solana/web3.js";

import {
  buildRequestNonce,
  normalizeSmokeTestConfig,
  parseDevnetSmokeTestArgs,
} from "../scripts/phase1/devnet-smoketest";
import {
  buildCompactReceiptMetadata,
  buildCompactSubmitArgs,
  buildReceiptHashMemcmpFilter,
  findReceiptByHash,
  RECEIPT_ACCOUNT_DISCRIMINATOR,
  RECEIPT_ACCOUNT_DISCRIMINATOR_BYTES,
  receiptHashHex,
  RECEIPT_HASH_MEMCMP_OFFSET,
} from "../scripts/phase1/compact-receipt";

describe("devnet-smoketest parser", () => {
  it("requires both deployment and config paths", () => {
    expect(() =>
      parseDevnetSmokeTestArgs([
        "--deployment",
        "deployments/devnet-phase1.json",
      ])
    ).to.throw("config path is required");
  });

  it("accepts an optional output path", () => {
    const args = parseDevnetSmokeTestArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--config",
      "configs/smoketest.devnet.json",
      "--out",
      "reports/smoke.json",
    ]);

    expect(args.deployment).to.equal("deployments/devnet-phase1.json");
    expect(args.config).to.equal("configs/smoketest.devnet.json");
    expect(args.out).to.equal("reports/smoke.json");
  });
});

describe("devnet-smoketest config", () => {
  it("normalizes defaults and converts the charge amount to base units", () => {
    const config = normalizeSmokeTestConfig({
      adminKeypair: "/tmp/admin.json",
      providerWalletKeypair: "/tmp/provider.json",
      providerSignerKeypair: "/tmp/provider-signer.json",
      payerKeypair: "/tmp/payer.json",
      negativeProviderWalletKeypair: "/tmp/negative-provider.json",
      providerCode: "u",
      receiptChargeUiAmount: "12.5",
    });

    expect(config.attesterTypeMask).to.equal(2);
    expect(config.requestNoncePrefix).to.equal("s");
    expect(config.proofId).to.equal("r");
    expect(config.model).to.equal("m");
    expect(config.promptTokens).to.equal(123);
    expect(config.completionTokens).to.equal(456);
    expect(config).to.not.have.property("totalTokens");
    expect(config.receiptChargeBaseUnits.toString()).to.equal("12500000");
  });

  it("aliases the old verbose smoke defaults to compact values", () => {
    const config = normalizeSmokeTestConfig({
      adminKeypair: "/tmp/admin.json",
      providerWalletKeypair: "/tmp/provider.json",
      providerSignerKeypair: "/tmp/provider-signer.json",
      payerKeypair: "/tmp/payer.json",
      negativeProviderWalletKeypair: "/tmp/negative-provider.json",
      providerCode: "u",
      receiptChargeUiAmount: "10",
      requestNoncePrefix: "smoke",
      proofId: "smoke-proof",
      model: "smoke-model",
    });

    expect(config.requestNoncePrefix).to.equal("s");
    expect(config.proofId).to.equal("r");
    expect(config.model).to.equal("m");
  });

  it("builds short nonces for transaction size safety", () => {
    const nonce = buildRequestNonce("s", "m", 1700000000000);

    expect(nonce).to.match(/^s-m-[0-9a-z]+$/);
    expect(nonce.length).to.be.lessThan(16);
  });

  it("requires the negative provider wallet for wrong-mint validation", () => {
    expect(() =>
      normalizeSmokeTestConfig({
        adminKeypair: "/tmp/admin.json",
        providerWalletKeypair: "/tmp/provider.json",
        providerSignerKeypair: "/tmp/provider-signer.json",
        payerKeypair: "/tmp/payer.json",
        providerCode: "u",
        receiptChargeUiAmount: "10",
      })
    ).to.throw("negativeProviderWalletKeypair is required");
  });
});

describe("compact receipt helpers", () => {
  const providerWallet = new PublicKey("11111111111111111111111111111111");
  const payerUser = new PublicKey("So11111111111111111111111111111111111111112");
  const usdcMint = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
  const bn = (value: bigint) => BigInt(value.toString());

  it("fills default compact receipt metadata fields once", () => {
    expect(
      buildCompactReceiptMetadata({
        proofId: "r",
        providerCode: "u",
        model: "m",
      })
    ).to.deep.equal({
      schema: "clawfarm-receipt-metadata/v2",
      proofId: "r",
      providerCode: "u",
      proofMode: "sig_log",
      attesterType: "gateway",
      usageBasis: "provider_reported",
      model: "m",
    });
  });

  it("builds deterministic receipt hashes for equivalent logical metadata", () => {
    const first = buildCompactSubmitArgs(
      {
        requestNonce: "s-o-k7m89",
        metadata: {
          proofId: "r",
          providerCode: "u",
          model: "m",
          providerRequestId: "request-1",
          issuedAt: 1700000000,
        },
        providerWallet,
        payerUser,
        usdcMint,
        promptTokens: 123,
        completionTokens: 456,
        chargeAtomic: BigInt(10_000_000),
      },
      bn
    );
    const second = buildCompactSubmitArgs(
      {
        requestNonce: "s-o-k7m89",
        metadata: {
          providerRequestId: "request-1",
          model: "m",
          proofId: "r",
          providerCode: "u",
          issuedAt: 1700000000,
        },
        providerWallet,
        payerUser,
        usdcMint,
        promptTokens: 123,
        completionTokens: 456,
        chargeAtomic: 10_000_000,
      },
      bn
    );

    expect(first.receiptHash).to.deep.equal(second.receiptHash);
    expect(receiptHashHex(first.receiptHash)).to.equal(receiptHashHex(second.receiptHash));
  });

  it("rejects non-integer numeric fields with labeled errors", () => {
    expect(() =>
      buildCompactSubmitArgs(
        {
          requestNonce: "s-o-k7m89",
          metadata: buildCompactReceiptMetadata({
            proofId: "r",
            providerCode: "u",
            model: "m",
          }),
          providerWallet,
          payerUser,
          usdcMint,
          promptTokens: 12.5,
          completionTokens: 456,
          chargeAtomic: 10_000_000,
        },
        bn
      )
    ).to.throw("promptTokens must be a non-negative safe integer");
  });

  it("queries receipt accounts by receipt_hash memcmp filter", async () => {
    const expectedReceipt = new PublicKey("2Z8j6Ao16Yh2fQ4M8pU6XqVkf6x32e6MFK7E1iWbzhfN");
    const attestationProgramId = new PublicKey(
      "6M4jM8NV3xncUgnPZXoqBYwygJyGTdQtdgQXVc3k5i3K"
    );
    const receiptHash = Uint8Array.from({ length: 32 }, (_, index) => index + 1);

    let observedFilter: unknown;
    const fakeConnection = {
      getProgramAccounts: async (_programId: PublicKey, config: unknown) => {
        observedFilter = config;
        return [{ pubkey: expectedReceipt, account: {} }];
      },
    } as unknown as Connection;

    const found = await findReceiptByHash(
      fakeConnection,
      attestationProgramId,
      receiptHash
    );
    expect(found?.toBase58()).to.equal(expectedReceipt.toBase58());
    expect(RECEIPT_ACCOUNT_DISCRIMINATOR).to.have.length(8);
    expect(observedFilter).to.deep.equal({
      filters: [
        {
          memcmp: {
            offset: 0,
            bytes: RECEIPT_ACCOUNT_DISCRIMINATOR_BYTES,
          },
        },
        {
          memcmp: {
            offset: RECEIPT_HASH_MEMCMP_OFFSET,
            bytes: buildReceiptHashMemcmpFilter(receiptHash).memcmp.bytes,
          },
        },
      ],
    });
  });
});
