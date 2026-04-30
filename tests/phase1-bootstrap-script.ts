import { expect } from "chai";

import { parseBootstrapArgs } from "../scripts/phase1/bootstrap-testnet";

describe("bootstrap-testnet parser", () => {
  it("requires separate admin and test usdc operator keypairs", () => {
    expect(() =>
      parseBootstrapArgs([
        "--cluster",
        "devnet",
        "--rpc-url",
        "https://api.devnet.solana.com",
        "--admin-keypair",
        "/tmp/admin.json",
        "--test-usdc-operator-keypair",
        "/tmp/admin.json",
        "--masterpool-program-id",
        "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux",
        "--attestation-program-id",
        "52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2",
        "--out",
        "deployments/devnet-phase1.json",
      ])
    ).to.throw("must differ");
  });

  it("defaults the output path when --out is omitted", () => {
    const args = parseBootstrapArgs([
      "--cluster",
      "devnet",
      "--rpc-url",
      "https://api.devnet.solana.com",
      "--admin-keypair",
      "/tmp/admin.json",
      "--test-usdc-operator-keypair",
      "/tmp/operator.json",
      "--masterpool-program-id",
      "AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux",
      "--attestation-program-id",
      "52WWsrQQcpAJn4cjSxMe4XGBvgGzPXa9gjAqUSfryAx2",
    ]);

    expect(args.out).to.equal("deployments/devnet-phase1.json");
  });
});
