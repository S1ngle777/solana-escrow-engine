/**
 * Devnet demo: creates a real escrow, funds it, and releases it.
 * Run: CLUSTER=devnet npx ts-node client/devnet_demo.ts
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

async function main() {
  const payer = loadKeypair();
  // Use different keypairs generated locally (no airdrop needed — payer covers everything)
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
  console.log(`\nWallet: ${payer.publicKey.toBase58()}`);
  console.log(`Balance: ${bal / LAMPORTS_PER_SOL} SOL`);
  console.log(`Escrow ID: ${ESCROW_ID.toString()}`);
  console.log(`Amount: ${AMOUNT.toNumber() / LAMPORTS_PER_SOL} SOL`);

  // ── 1. Initialize ──────────────────────────────────────────────────────────
  console.log("\n[1/3] Calling initializeEscrow...");
  const tx1 = await (program.methods as any)
    .initializeEscrow(
      ESCROW_ID,
      seller.publicKey,
      arbiter.publicKey,
      AMOUNT,
      "Devnet demo: payment for bounty submission"
    )
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      systemProgram: SystemProgram.programId,
    })
    .signers([payer])
    .rpc();
  console.log(`✅ initializeEscrow: https://explorer.solana.com/tx/${tx1}?cluster=devnet`);

  // ── 2. Deposit ─────────────────────────────────────────────────────────────
  console.log("\n[2/3] Calling deposit...");
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
  console.log(`✅ deposit: https://explorer.solana.com/tx/${tx2}?cluster=devnet`);

  // ── 3. Release ─────────────────────────────────────────────────────────────
  console.log("\n[3/3] Calling release (buyer approves)...");
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
  console.log(`✅ release: https://explorer.solana.com/tx/${tx3}?cluster=devnet`);

  // ── Read final state ───────────────────────────────────────────────────────
  const escrow = await (program.account as any).escrowAccount.fetch(escrowPda);
  console.log("\n📋 Final escrow state:", JSON.stringify(escrow.state));
  console.log("\n═══════════════════════════════════════════════════");
  console.log("DEVNET TRANSACTION LINKS:");
  console.log(`Deploy:   https://explorer.solana.com/tx/RCjdWhimdFD78YP4pSSC98bLp28u4csDQ41JiEjFowM9EWjYHknLGK1AU7Msi1JdsnowGXtH1Hhj7mPGcvn7zXR?cluster=devnet`);
  console.log(`Init:     https://explorer.solana.com/tx/${tx1}?cluster=devnet`);
  console.log(`Deposit:  https://explorer.solana.com/tx/${tx2}?cluster=devnet`);
  console.log(`Release:  https://explorer.solana.com/tx/${tx3}?cluster=devnet`);
  console.log(`Program:  https://explorer.solana.com/address/${PROGRAM_ID.toBase58()}?cluster=devnet`);
  console.log("═══════════════════════════════════════════════════");
}

main().catch(console.error);
