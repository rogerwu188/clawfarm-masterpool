import { expect } from "chai";
import { PublicKey } from "@solana/web3.js";

import {
  PHASE1_70_30_PARAMS,
  parseUpdateConfigArgs,
} from "../scripts/phase1/update-config";

describe("update-config parser", () => {
  it("requires deployment and admin keypair paths", () => {
    expect(() =>
      parseUpdateConfigArgs([
        "--deployment",
        "deployments/devnet-phase1.json",
      ])
    ).to.throw("admin keypair path is required");
  });

  it("defaults to the Provider 70% and treasury 30% USDC split", () => {
    expect(PHASE1_70_30_PARAMS.providerUsdcShareBps).to.equal(700);
    expect(PHASE1_70_30_PARAMS.treasuryUsdcShareBps).to.equal(300);
  });

  it("accepts optional rpc and program id overrides", () => {
    const args = parseUpdateConfigArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--admin-keypair",
      "/tmp/admin.json",
      "--rpc-url",
      "https://api.devnet.solana.com",
      "--masterpool-program-id",
      "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux",
    ]);

    expect(args.deployment).to.equal("deployments/devnet-phase1.json");
    expect(args.adminKeypair).to.equal("/tmp/admin.json");
    expect(args.rpcUrl).to.equal("https://api.devnet.solana.com");
    expect(args.masterpoolProgramId).to.deep.equal(
      new PublicKey("AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux")
    );
  });
});
