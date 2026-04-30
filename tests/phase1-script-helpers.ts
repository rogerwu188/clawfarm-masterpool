import { expect } from "chai";
import { PublicKey } from "@solana/web3.js";

import {
  deriveMasterpoolPdas,
  toBaseUnits,
} from "../scripts/phase1/common";

describe("phase1 script helpers", () => {
  it("derives the same pool authority PDA as the on-chain seed", () => {
    const programId = new PublicKey("AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux");
    const expectedPoolAuthority = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_authority")],
      programId
    )[0];

    expect(deriveMasterpoolPdas(programId).poolAuthority.toBase58()).to.equal(
      expectedPoolAuthority.toBase58()
    );
  });

  it("parses six-decimal ui amounts into base units", () => {
    expect(toBaseUnits("1", 6).toString()).to.equal("1000000");
    expect(toBaseUnits("12.345678", 6).toString()).to.equal("12345678");
  });

  it("rejects ui amounts with too many fractional digits", () => {
    expect(() => toBaseUnits("0.0000001", 6)).to.throw(
      "too many decimal places"
    );
  });

  it("derives faucet PDAs", () => {
    const programId = new PublicKey("AP5gMEh6yHjvZBjh7Xg5fgs4EnBiCbVUoDyXxMi1omux");
    const pdas = deriveMasterpoolPdas(programId);

    expect(pdas.faucetConfig.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_config")], programId)[0].toBase58()
    );
    expect(pdas.faucetGlobal.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_global")], programId)[0].toBase58()
    );
    expect(pdas.faucetClawVault.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_claw_vault")], programId)[0].toBase58()
    );
    expect(pdas.faucetUsdcVault.toBase58()).to.equal(
      PublicKey.findProgramAddressSync([Buffer.from("faucet_usdc_vault")], programId)[0].toBase58()
    );
  });
});
