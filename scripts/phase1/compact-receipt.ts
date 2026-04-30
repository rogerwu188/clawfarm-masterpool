import crypto from "crypto";
import bs58 from "bs58";
import { Connection, PublicKey } from "@solana/web3.js";

export const COMPACT_RECEIPT_DOMAIN_SEPARATOR = "clawfarm:receipt:v2";
export const COMPACT_RECEIPT_METADATA_SCHEMA = "clawfarm-receipt-metadata/v2";
export const RECEIPT_ACCOUNT_DISCRIMINATOR_OFFSET = 0;
export const RECEIPT_HASH_MEMCMP_OFFSET = 8;
export const RECEIPT_ACCOUNT_DISCRIMINATOR = sha256(
  Buffer.from("account:Receipt", "utf8")
).subarray(0, 8);
export const RECEIPT_ACCOUNT_DISCRIMINATOR_BYTES = bs58.encode(
  RECEIPT_ACCOUNT_DISCRIMINATOR
);

export interface CompactReceiptMetadataInput {
  schema?: string;
  proofId: string;
  providerCode: string;
  proofMode?: string;
  attesterType?: string;
  usageBasis?: string;
  model: string;
  providerRequestId?: string;
  issuedAt?: number;
  expiresAt?: number;
}

export interface CompactReceiptMetadata {
  schema: string;
  proofId: string;
  providerCode: string;
  proofMode: string;
  attesterType: string;
  usageBasis: string;
  model: string;
  providerRequestId?: string;
  issuedAt?: number;
  expiresAt?: number;
}

export interface CompactReceiptHashInputs {
  requestNonceHash: Uint8Array;
  metadataHash: Uint8Array;
  providerWallet: PublicKey;
  payerUser: PublicKey;
  usdcMint: PublicKey;
  promptTokens: number | bigint;
  completionTokens: number | bigint;
  chargeAtomic: number | bigint;
}

export interface CompactSubmitArgsInput {
  requestNonce: string;
  metadata: CompactReceiptMetadataInput;
  providerWallet: PublicKey;
  payerUser: PublicKey;
  usdcMint: PublicKey;
  promptTokens: number | bigint;
  completionTokens: number | bigint;
  chargeAtomic: number | bigint;
}

export function hashRequestNonce(requestNonce: string): Buffer {
  return sha256(Buffer.from(requestNonce, "utf8"));
}

export function buildCompactReceiptMetadata(
  input: CompactReceiptMetadataInput
): CompactReceiptMetadata {
  return {
    schema: input.schema ?? COMPACT_RECEIPT_METADATA_SCHEMA,
    proofId: input.proofId,
    providerCode: input.providerCode,
    proofMode: input.proofMode ?? "sig_log",
    attesterType: input.attesterType ?? "gateway",
    usageBasis: input.usageBasis ?? "provider_reported",
    model: input.model,
    ...(input.providerRequestId !== undefined
      ? { providerRequestId: input.providerRequestId }
      : {}),
    ...(input.issuedAt !== undefined ? { issuedAt: input.issuedAt } : {}),
    ...(input.expiresAt !== undefined ? { expiresAt: input.expiresAt } : {}),
  };
}

export function stableMetadataJson(metadata: CompactReceiptMetadata): string {
  return canonicalizeJson({
    schema: metadata.schema,
    proof_id: metadata.proofId,
    provider_code: metadata.providerCode,
    proof_mode: metadata.proofMode,
    attester_type: metadata.attesterType,
    usage_basis: metadata.usageBasis,
    model: metadata.model,
    provider_request_id: metadata.providerRequestId,
    issued_at: metadata.issuedAt,
    expires_at: metadata.expiresAt,
  });
}

export function hashReceiptMetadata(
  metadata: CompactReceiptMetadataInput | CompactReceiptMetadata
): Buffer {
  return sha256(
    Buffer.from(stableMetadataJson(buildCompactReceiptMetadata(metadata)), "utf8")
  );
}

export function buildCompactReceiptHash(inputs: CompactReceiptHashInputs): Buffer {
  const promptTokens = normalizeU64(inputs.promptTokens, "promptTokens");
  const completionTokens = normalizeU64(inputs.completionTokens, "completionTokens");
  const chargeAtomic = normalizeU64(inputs.chargeAtomic, "chargeAtomic");

  return sha256(
    Buffer.concat([
      Buffer.from(COMPACT_RECEIPT_DOMAIN_SEPARATOR, "utf8"),
      normalizeHash(inputs.requestNonceHash, "requestNonceHash"),
      normalizeHash(inputs.metadataHash, "metadataHash"),
      inputs.providerWallet.toBuffer(),
      inputs.payerUser.toBuffer(),
      inputs.usdcMint.toBuffer(),
      u64Le(promptTokens),
      u64Le(completionTokens),
      u64Le(chargeAtomic),
    ])
  );
}

export function receiptHashHex(receiptHash: Uint8Array | number[]): string {
  return `0x${Buffer.from(normalizeHash(receiptHash, "receiptHash")).toString("hex")}`;
}

export function buildReceiptHashMemcmpFilter(receiptHash: Uint8Array | number[]): {
  memcmp: {
    offset: number;
    bytes: string;
  };
} {
  return {
    memcmp: {
      offset: RECEIPT_HASH_MEMCMP_OFFSET,
      bytes: bs58.encode(normalizeHash(receiptHash, "receiptHash")),
    },
  };
}

export function buildCompactSubmitArgs<TBn>(
  input: CompactSubmitArgsInput,
  toBn: (value: bigint) => TBn
): {
  requestNonceHash: number[];
  metadataHash: number[];
  promptTokens: TBn;
  completionTokens: TBn;
  chargeAtomic: TBn;
  receiptHash: number[];
} {
  const promptTokens = normalizeU64(input.promptTokens, "promptTokens");
  const completionTokens = normalizeU64(input.completionTokens, "completionTokens");
  const chargeAtomic = normalizeU64(input.chargeAtomic, "chargeAtomic");

  const requestNonceHash = hashRequestNonce(input.requestNonce);
  const metadata = buildCompactReceiptMetadata(input.metadata);
  const metadataHash = hashReceiptMetadata(metadata);
  const receiptHash = buildCompactReceiptHash({
    requestNonceHash,
    metadataHash,
    providerWallet: input.providerWallet,
    payerUser: input.payerUser,
    usdcMint: input.usdcMint,
    promptTokens,
    completionTokens,
    chargeAtomic,
  });

  return {
    requestNonceHash: Array.from(requestNonceHash),
    metadataHash: Array.from(metadataHash),
    promptTokens: toBn(promptTokens),
    completionTokens: toBn(completionTokens),
    chargeAtomic: toBn(chargeAtomic),
    receiptHash: Array.from(receiptHash),
  };
}

export async function findReceiptByHash(
  connection: Connection,
  attestationProgramId: PublicKey,
  receiptHash: Uint8Array | number[]
): Promise<PublicKey | null> {
  const hash = normalizeHash(receiptHash, "receiptHash");
  const matches = await connection.getProgramAccounts(attestationProgramId, {
    filters: [
      {
        memcmp: {
          offset: RECEIPT_ACCOUNT_DISCRIMINATOR_OFFSET,
          bytes: RECEIPT_ACCOUNT_DISCRIMINATOR_BYTES,
        },
      },
      buildReceiptHashMemcmpFilter(hash),
    ],
  });

  if (matches.length > 1) {
    throw new Error(
      `receipt hash lookup returned ${matches.length} matching receipt accounts`
    );
  }
  return matches[0]?.pubkey ?? null;
}

function normalizeHash(value: Uint8Array | number[], label: string): Buffer {
  const bytes = Buffer.from(value);
  if (bytes.length !== 32) {
    throw new Error(`${label} must be exactly 32 bytes`);
  }
  return bytes;
}

function normalizeU64(value: number | bigint, label: string): bigint {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${label} must be a non-negative safe integer`);
    }
    return assertU64(BigInt(value), label);
  }
  return assertU64(value, label);
}

function assertU64(value: bigint, label: string): bigint {
  if (value < BigInt(0) || value > BigInt("18446744073709551615")) {
    throw new Error(`${label} must fit in u64`);
  }
  return value;
}

function canonicalizeJson(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error("metadata numbers must be finite");
    }
    return JSON.stringify(value);
  }
  if (typeof value === "boolean") {
    return value ? "true" : "false";
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalizeJson(item)).join(",")}]`;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, entryValue]) => entryValue !== undefined)
      .sort(([left], [right]) => compareCanonicalKeys(left, right));
    return `{${entries
      .map(
        ([key, entryValue]) =>
          `${JSON.stringify(key)}:${canonicalizeJson(entryValue)}`
      )
      .join(",")}}`;
  }
  throw new Error(`unsupported metadata value type: ${typeof value}`);
}

function compareCanonicalKeys(left: string, right: string): number {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

function u64Le(value: bigint): Buffer {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(value);
  return out;
}

function sha256(data: Buffer): Buffer {
  return crypto.createHash("sha256").update(data).digest();
}
