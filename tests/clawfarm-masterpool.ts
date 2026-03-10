import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ClawfarmMasterpool } from "../target/types/clawfarm_masterpool";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint } from "@solana/spl-token";
import { assert } from "chai";

describe("clawfarm-masterpool", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.ClawfarmMasterpool as Program<ClawfarmMasterpool>;

  let clawMint: PublicKey;
  let usdcMint: PublicKey;
  let configPda: PublicKey;
  let masterPoolVaultPda: PublicKey;
  let treasuryVaultPda: PublicKey;
  let poolAuthorityPda: PublicKey;

  const deployer = provider.wallet;
  const adminMultisig = Keypair.generate().publicKey;
  const timelockAuthority = Keypair.generate().publicKey;

  before(async () => {
    // Derive PDAs
    [configPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );
    [masterPoolVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("master_pool_vault")],
      program.programId
    );
    [treasuryVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury_vault")],
      program.programId
    );
    [poolAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_authority")],
      program.programId
    );

    // Create CLAW mint with pool_authority as mint authority
    clawMint = await createMint(
      provider.connection,
      (deployer as any).payer,
      poolAuthorityPda,  // mint authority = PDA
      poolAuthorityPda,  // freeze authority = PDA
      6                  // 6 decimals
    );

    // Create USDC mock mint
    usdcMint = await createMint(
      provider.connection,
      (deployer as any).payer,
      deployer.publicKey,
      null,
      6
    );
  });

  it("Phase A.1: Initialize Master Pool", async () => {
    await program.methods
      .initializeMasterPool(adminMultisig, timelockAuthority)
      .accounts({
        config: configPda,
        clawMint: clawMint,
        deployer: deployer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.equal(config.version, 1);
    assert.equal(config.isInitialized, true);
    assert.equal(config.computePoolBps, 5000);
    assert.equal(config.outcomePoolBps, 5000);
    assert.equal(config.treasuryTaxBps, 300);
    assert.equal(config.genesisMinted, false);
    assert.equal(config.settlementEnabled, false);
    console.log("✅ Config PDA:", configPda.toString());
  });

  it("Phase A.2: Create Master Pool Vault", async () => {
    await program.methods
      .createMasterPoolVault()
      .accounts({
        config: configPda,
        masterPoolVault: masterPoolVaultPda,
        poolAuthority: poolAuthorityPda,
        clawMint: clawMint,
        deployer: deployer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.ok(config.masterPoolVault.equals(masterPoolVaultPda));
    console.log("✅ Master Pool Vault:", masterPoolVaultPda.toString());
  });

  it("Phase A.3: Create Treasury Vault", async () => {
    await program.methods
      .createTreasuryVault()
      .accounts({
        config: configPda,
        treasuryVault: treasuryVaultPda,
        poolAuthority: poolAuthorityPda,
        usdcMint: usdcMint,
        deployer: deployer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.ok(config.treasuryVault.equals(treasuryVaultPda));
    console.log("✅ Treasury Vault:", treasuryVaultPda.toString());
  });

  it("Phase B.1: Mint Genesis Supply", async () => {
    await program.methods
      .mintGenesisSupply()
      .accounts({
        config: configPda,
        clawMint: clawMint,
        masterPoolVault: masterPoolVaultPda,
        poolAuthority: poolAuthorityPda,
        deployer: deployer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.equal(config.genesisMinted, true);
    console.log("✅ Genesis minted: 1,000,000,000 CLAW");
  });

  it("Phase B.2: Cannot mint again", async () => {
    try {
      await program.methods
        .mintGenesisSupply()
        .accounts({
          config: configPda,
          clawMint: clawMint,
          masterPoolVault: masterPoolVaultPda,
          poolAuthority: poolAuthorityPda,
          deployer: deployer.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      assert.fail("Should have thrown");
    } catch (e: any) {
      assert.include(e.toString(), "GenesisAlreadyMinted");
      console.log("✅ Double mint correctly rejected");
    }
  });

  it("Phase B.3: Revoke Mint Authority", async () => {
    await program.methods
      .revokeMintAuthority()
      .accounts({
        config: configPda,
        clawMint: clawMint,
        poolAuthority: poolAuthorityPda,
        deployer: deployer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.equal(config.mintAuthorityRevoked, true);
    console.log("✅ Mint authority permanently revoked");
  });

  it("Phase B.4: Revoke Freeze Authority", async () => {
    await program.methods
      .revokeFreezeAuthority()
      .accounts({
        config: configPda,
        clawMint: clawMint,
        poolAuthority: poolAuthorityPda,
        deployer: deployer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const config = await program.account.clawFarmConfig.fetch(configPda);
    assert.equal(config.freezeAuthorityRevoked, true);
    console.log("✅ Freeze authority permanently revoked");
  });

  it("Summary: Print all addresses", async () => {
    console.log("\n═══════════════════════════════════════");
    console.log("  ClawFarm Master Pool — Phase A+B Complete");
    console.log("═══════════════════════════════════════");
    console.log("Program ID:         ", program.programId.toString());
    console.log("Config PDA:         ", configPda.toString());
    console.log("Master Pool Vault:  ", masterPoolVaultPda.toString());
    console.log("Treasury Vault:     ", treasuryVaultPda.toString());
    console.log("Pool Authority:     ", poolAuthorityPda.toString());
    console.log("CLAW Mint:          ", clawMint.toString());
    console.log("═══════════════════════════════════════\n");
  });
});
