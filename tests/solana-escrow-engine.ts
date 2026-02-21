import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { SolanaEscrowEngine } from "../target/types/solana_escrow_engine";
import { assert } from "chai";

// ─── Helpers ─────────────────────────────────────────────────────────────────

async function airdrop(
  provider: anchor.AnchorProvider,
  pubkey: PublicKey,
  sol: number
) {
  const sig = await provider.connection.requestAirdrop(
    pubkey,
    sol * LAMPORTS_PER_SOL
  );
  await provider.connection.confirmTransaction(sig, "confirmed");
}

function getPDAs(
  program: Program<SolanaEscrowEngine>,
  buyer: PublicKey,
  escrowId: BN
) {
  const idBytes = escrowId.toArrayLike(Buffer, "le", 8);
  const [escrowPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), buyer.toBuffer(), idBytes],
    program.programId
  );
  const [vaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), buyer.toBuffer(), idBytes],
    program.programId
  );
  return { escrowPda, vaultPda };
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe("solana-escrow-engine", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .SolanaEscrowEngine as Program<SolanaEscrowEngine>;

  // Wallets
  const buyer = Keypair.generate();
  const seller = Keypair.generate();
  const arbiter = Keypair.generate();
  const outsider = Keypair.generate();

  const ESCROW_ID = new BN(1);
  const AMOUNT = new BN(0.1 * LAMPORTS_PER_SOL); // 0.1 SOL

  let escrowPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Airdrop to all participants
    await Promise.all([
      airdrop(provider, buyer.publicKey, 5),
      airdrop(provider, seller.publicKey, 1),
      airdrop(provider, arbiter.publicKey, 1),
      airdrop(provider, outsider.publicKey, 1),
    ]);
    ({ escrowPda, vaultPda } = getPDAs(program, buyer.publicKey, ESCROW_ID));
    console.log("  Program ID:", program.programId.toBase58());
    console.log("  Buyer:     ", buyer.publicKey.toBase58());
    console.log("  Seller:    ", seller.publicKey.toBase58());
    console.log("  Arbiter:   ", arbiter.publicKey.toBase58());
    console.log("  Escrow PDA:", escrowPda.toBase58());
    console.log("  Vault PDA: ", vaultPda.toBase58());
  });

  // ─── Test 1: Initialize ────────────────────────────────────────────────────
  it("1. Initializes an escrow (buyer creates PDA accounts)", async () => {
    const tx = await program.methods
      .initializeEscrow(
        ESCROW_ID,
        seller.publicKey,
        arbiter.publicKey,
        AMOUNT,
        "Payment for freelance web development work"
      )
      .accounts({
        buyer: buyer.publicKey,
        escrow: escrowPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    console.log("  ✓ initializeEscrow tx:", tx);

    // Verify on-chain state
    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    assert.equal(escrow.escrowId.toString(), ESCROW_ID.toString());
    assert.equal(escrow.buyer.toBase58(), buyer.publicKey.toBase58());
    assert.equal(escrow.seller.toBase58(), seller.publicKey.toBase58());
    assert.equal(escrow.arbiter.toBase58(), arbiter.publicKey.toBase58());
    assert.equal(escrow.amount.toString(), AMOUNT.toString());
    assert.equal(escrow.description, "Payment for freelance web development work");
    assert.deepEqual(escrow.state, { active: {} });
  });

  // ─── Test 2: Deposit ───────────────────────────────────────────────────────
  it("2. Deposits SOL into vault (buyer funds the escrow)", async () => {
    const vaultBefore = await provider.connection.getBalance(vaultPda);
    const buyerBefore = await provider.connection.getBalance(buyer.publicKey);

    const tx = await program.methods
      .deposit(ESCROW_ID)
      .accounts({
        buyer: buyer.publicKey,
        escrow: escrowPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    console.log("  ✓ deposit tx:", tx);

    const vaultAfter = await provider.connection.getBalance(vaultPda);
    const escrow = await program.account.escrowAccount.fetch(escrowPda);

    // Vault should have gained exactly AMOUNT lamports
    assert.equal(vaultAfter - vaultBefore, AMOUNT.toNumber());
    assert.deepEqual(escrow.state, { funded: {} });
    console.log(`  Vault balance: ${vaultAfter} lamports (+${AMOUNT} escrowed)`);
  });

  // ─── Test 3: Unauthorized dispute ─────────────────────────────────────────
  it("3. Rejects dispute from an outsider (access control check)", async () => {
    try {
      await program.methods
        .dispute(ESCROW_ID)
        .accounts({
          caller: outsider.publicKey,
          escrow: escrowPda,
        })
        .signers([outsider])
        .rpc();
      assert.fail("Should have thrown Unauthorized error");
    } catch (err: any) {
      assert.include(err.message, "Unauthorized");
      console.log("  ✓ Outsider correctly rejected with Unauthorized");
    }
  });

  // ─── Test 4: Dispute ───────────────────────────────────────────────────────
  it("4. Buyer opens a dispute (state transition: Funded → Disputed)", async () => {
    const tx = await program.methods
      .dispute(ESCROW_ID)
      .accounts({
        caller: buyer.publicKey,
        escrow: escrowPda,
      })
      .signers([buyer])
      .rpc();

    console.log("  ✓ dispute tx:", tx);

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    assert.deepEqual(escrow.state, { disputed: {} });
  });

  // ─── Test 5: Arbiter resolves — releases to seller ─────────────────────────
  it("5. Arbiter releases funds to seller (dispute resolution)", async () => {
    const sellerBefore = await provider.connection.getBalance(seller.publicKey);
    const vaultBefore = await provider.connection.getBalance(vaultPda);

    const tx = await program.methods
      .release(ESCROW_ID)
      .accounts({
        caller: arbiter.publicKey,
        escrow: escrowPda,
        vault: vaultPda,
        seller: seller.publicKey,
      })
      .signers([arbiter])
      .rpc();

    console.log("  ✓ release tx:", tx);

    const sellerAfter = await provider.connection.getBalance(seller.publicKey);
    const vaultAfter = await provider.connection.getBalance(vaultPda);
    const escrow = await program.account.escrowAccount.fetch(escrowPda);

    assert.equal(sellerAfter - sellerBefore, AMOUNT.toNumber());
    assert.equal(vaultBefore - vaultAfter, AMOUNT.toNumber());
    assert.deepEqual(escrow.state, { released: {} });
    console.log(`  Seller received: ${sellerAfter - sellerBefore} lamports`);
  });

  // ─── Test 6: Happy path refund ─────────────────────────────────────────────
  it("6. Seller voluntarily refunds buyer (happy path refund)", async () => {
    const escrowId2 = new BN(2);
    const { escrowPda: ep2, vaultPda: vp2 } = getPDAs(
      program,
      buyer.publicKey,
      escrowId2
    );

    // Create + fund a new escrow
    await program.methods
      .initializeEscrow(
        escrowId2,
        seller.publicKey,
        arbiter.publicKey,
        AMOUNT,
        "Second escrow for refund test"
      )
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep2,
        vault: vp2,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    await program.methods
      .deposit(escrowId2)
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep2,
        vault: vp2,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    const buyerBefore = await provider.connection.getBalance(buyer.publicKey);

    const tx = await program.methods
      .refund(escrowId2)
      .accounts({
        caller: seller.publicKey,
        escrow: ep2,
        vault: vp2,
        buyer: buyer.publicKey,
      })
      .signers([seller])
      .rpc();

    console.log("  ✓ refund tx:", tx);

    const buyerAfter = await provider.connection.getBalance(buyer.publicKey);
    const escrowState = await program.account.escrowAccount.fetch(ep2);

    assert.equal(buyerAfter - buyerBefore, AMOUNT.toNumber());
    assert.deepEqual(escrowState.state, { refunded: {} });
    console.log(`  Buyer refunded: ${buyerAfter - buyerBefore} lamports`);
  });

  // ─── Test 7: Double-deposit rejected ──────────────────────────────────────
  it("7. Rejects double-deposit on already funded escrow (state guard)", async () => {
    const escrowId3 = new BN(3);
    const { escrowPda: ep3, vaultPda: vp3 } = getPDAs(
      program,
      buyer.publicKey,
      escrowId3
    );

    await program.methods
      .initializeEscrow(
        escrowId3,
        seller.publicKey,
        arbiter.publicKey,
        AMOUNT,
        "Double-deposit test"
      )
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep3,
        vault: vp3,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    await program.methods
      .deposit(escrowId3)
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep3,
        vault: vp3,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    try {
      await program.methods
        .deposit(escrowId3)
        .accounts({
          buyer: buyer.publicKey,
          escrow: ep3,
          vault: vp3,
          systemProgram: SystemProgram.programId,
        })
        .signers([buyer])
        .rpc();
      assert.fail("Should have thrown InvalidState");
    } catch (err: any) {
      assert.include(err.message, "InvalidState");
      console.log("  ✓ Double-deposit correctly rejected with InvalidState");
    }
  });

  // ─── Test 8: Buyer releases directly ──────────────────────────────────────
  it("8. Buyer releases funds directly to seller (happy path)", async () => {
    const escrowId4 = new BN(4);
    const { escrowPda: ep4, vaultPda: vp4 } = getPDAs(
      program,
      buyer.publicKey,
      escrowId4
    );

    await program.methods
      .initializeEscrow(
        escrowId4,
        seller.publicKey,
        arbiter.publicKey,
        AMOUNT,
        "Happy path: buyer releases directly"
      )
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep4,
        vault: vp4,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    await program.methods
      .deposit(escrowId4)
      .accounts({
        buyer: buyer.publicKey,
        escrow: ep4,
        vault: vp4,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    const sellerBefore = await provider.connection.getBalance(seller.publicKey);

    const tx = await program.methods
      .release(escrowId4)
      .accounts({
        caller: buyer.publicKey,
        escrow: ep4,
        vault: vp4,
        seller: seller.publicKey,
      })
      .signers([buyer])
      .rpc();

    console.log("  ✓ buyer-release tx:", tx);

    const sellerAfter = await provider.connection.getBalance(seller.publicKey);
    const escrowState = await program.account.escrowAccount.fetch(ep4);

    assert.equal(sellerAfter - sellerBefore, AMOUNT.toNumber());
    assert.deepEqual(escrowState.state, { released: {} });
    console.log(`  Seller received: ${sellerAfter - sellerBefore} lamports`);
  });
});

