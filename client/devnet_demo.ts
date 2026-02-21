/**
 * Devnet Demo — Full Escrow Lifecycle
 *
 * Demonstrates all 6 instructions on Solana Devnet:
 *   1. initializeEscrow — create escrow + vault PDAs
 *   2. deposit          — fund the vault with SOL
 *   3. release          — buyer releases funds to seller
 *   4. closeEscrow      — reclaim rent from settled escrow
 *
 * Run: npx ts-node client/devnet_demo.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { BN, Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  Connection,
  SystemProgram,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

const RPC_URL = "https://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey("9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF");
const IDL = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../target/idl/solana_escrow_engine.json"),
    "utf-8"
  )
);

function loadKeypair(): Keypair {
  const raw = JSON.parse(
    fs.readFileSync(
      path.join(process.env.HOME || "", ".config/solana/id.json"),
      "utf-8"
    )
  );
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function getPDAs(buyer: PublicKey, escrowId: BN) {
  const b = escrowId.toArrayLike(Buffer, "le", 8);
  const [escrowPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), buyer.toBuffer(), b],
    PROGRAM_ID
  );
  const [vaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), buyer.toBuffer(), b],
    PROGRAM_ID
  );
  return { escrowPda, vaultPda };
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

async function main() {
  const payer = loadKeypair();
  const seller = Keypair.generate();
  const arbiter = Keypair.generate();

  const connection = new Connection(RPC_URL, "confirmed");
  const provider = new AnchorProvider(connection, new Wallet(payer), {
    commitment: "confirmed",
  });
  const program = new Program(IDL, provider);

  const ESCROW_ID = new BN(Date.now() % 100000); // unique id
  const AMOUNT = new BN(0.01 * LAMPORTS_PER_SOL); // 0.01 SOL
  const { escrowPda, vaultPda } = getPDAs(payer.publicKey, ESCROW_ID);

  const bal = await connection.getBalance(payer.publicKey);
  console.log(`\n╔═══════════════════════════════════════════════════╗`);
  console.log(`║         SOLANA ESCROW ENGINE — DEVNET DEMO        ║`);
  console.log(`╚═══════════════════════════════════════════════════╝`);
  console.log(`\n  Wallet:    ${payer.publicKey.toBase58()}`);
  console.log(`  Balance:   ${bal / LAMPORTS_PER_SOL} SOL`);
  console.log(`  Escrow ID: ${ESCROW_ID.toString()}`);
  console.log(`  Amount:    ${AMOUNT.toNumber() / LAMPORTS_PER_SOL} SOL`);
  console.log(`  Seller:    ${seller.publicKey.toBase58()}`);
  console.log(`  Arbiter:   ${arbiter.publicKey.toBase58()}`);

  // ── 1. Initialize ──────────────────────────────────────────────────────────
  console.log("\n[1/4] initializeEscrow — creating EscrowAccount + VaultAccount PDAs...");
  const tx1 = await (program.methods as any)
    .initializeEscrow(
      ESCROW_ID,
      seller.publicKey,
      arbiter.publicKey,
      AMOUNT,
      "Devnet demo: full escrow lifecycle with rent reclamation"
    )
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      systemProgram: SystemProgram.programId,
    })
    .signers([payer])
    .rpc();
  console.log(`  ✅ TX: https://explorer.solana.com/tx/${tx1}?cluster=devnet`);

  await sleep(1500);

  // ── 2. Deposit ─────────────────────────────────────────────────────────────
  console.log("\n[2/4] deposit — locking SOL in vault PDA...");
  const tx2 = await (program.methods as any)
    .deposit(ESCROW_ID)
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      systemProgram: SystemProgram.programId,
    })
    .signers([payer])
    .rpc();
  console.log(`  ✅ TX: https://explorer.solana.com/tx/${tx2}?cluster=devnet`);

  await sleep(1500);

  // ── 3. Release ─────────────────────────────────────────────────────────────
  console.log("\n[3/4] release — buyer approves, SOL transfers to seller...");
  const tx3 = await (program.methods as any)
    .release(ESCROW_ID)
    .accounts({
      caller: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      seller: seller.publicKey,
    })
    .signers([payer])
    .rpc();
  console.log(`  ✅ TX: https://explorer.solana.com/tx/${tx3}?cluster=devnet`);

  await sleep(1500);

  // ── 4. Close (rent reclamation) ────────────────────────────────────────────
  const rentBefore = await connection.getBalance(payer.publicKey);
  console.log("\n[4/4] closeEscrow — reclaiming rent from settled escrow...");
  const tx4 = await (program.methods as any)
    .closeEscrow(ESCROW_ID)
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
    })
    .signers([payer])
    .rpc();
  const rentAfter = await connection.getBalance(payer.publicKey);
  console.log(`  ✅ TX: https://explorer.solana.com/tx/${tx4}?cluster=devnet`);
  console.log(`  💰 Rent reclaimed: ${(rentAfter - rentBefore) / LAMPORTS_PER_SOL} SOL`);

  // ── Summary ────────────────────────────────────────────────────────────────
  console.log(`\n╔═══════════════════════════════════════════════════╗`);
  console.log(`║                DEVNET TRANSACTION LOG             ║`);
  console.log(`╠═══════════════════════════════════════════════════╣`);
  console.log(`║ Program: ${PROGRAM_ID.toBase58()}           ║`);
  console.log(`╠═══════════════════════════════════════════════════╣`);
  console.log(`║ initializeEscrow:                                ║`);
  console.log(`║   ${tx1}`);
  console.log(`║ deposit:                                         ║`);
  console.log(`║   ${tx2}`);
  console.log(`║ release:                                         ║`);
  console.log(`║   ${tx3}`);
  console.log(`║ closeEscrow:                                     ║`);
  console.log(`║   ${tx4}`);
  console.log(`╚═══════════════════════════════════════════════════╝`);
  console.log(`\nExplorer links:`);
  console.log(`  Program:  https://explorer.solana.com/address/${PROGRAM_ID.toBase58()}?cluster=devnet`);
  console.log(`  Init:     https://explorer.solana.com/tx/${tx1}?cluster=devnet`);
  console.log(`  Deposit:  https://explorer.solana.com/tx/${tx2}?cluster=devnet`);
  console.log(`  Release:  https://explorer.solana.com/tx/${tx3}?cluster=devnet`);
  console.log(`  Close:    https://explorer.solana.com/tx/${tx4}?cluster=devnet`);
}

main().catch(console.error);
