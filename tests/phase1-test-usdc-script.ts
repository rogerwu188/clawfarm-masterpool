import { expect } from "chai";

import { parseMintTestUsdcArgs } from "../scripts/phase1/mint-test-usdc";

describe("mint-test-usdc parser", () => {
  it("parses ui amounts with six decimals", () => {
    const args = parseMintTestUsdcArgs([
      "--deployment",
      "deployments/devnet-phase1.json",
      "--operator-keypair",
      "/tmp/operator.json",
      "--recipient",
      "11111111111111111111111111111111",
      "--amount",
      "15.250000",
    ]);

    expect(args.amountBaseUnits.toString()).to.equal("15250000");
  });

  it("requires a deployment record path", () => {
    expect(() =>
      parseMintTestUsdcArgs([
        "--operator-keypair",
        "/tmp/operator.json",
        "--recipient",
        "11111111111111111111111111111111",
        "--amount",
        "1",
      ])
    ).to.throw("deployment record is required");
  });
});
