#!/usr/bin/env ts-node
/**
 * Solana Escrow Engine - CLI Client
 *
 * Usage:
 *   ts-node client/cli.ts <command> [options]
 *
 * Commands:
 *   init    --id <n> --seller <pk> --arbiter <pk> --amount <sol> --desc <text>
 *   deposit --id <n> --buyer <pk>
 *   release --id <n> --buyer <pk> --seller <pk>   (caller = buyer or arbiter)
 *   refund  --id <n> --buyer <pk>                  (caller = seller or arbiter)
 *   dispute --id <n> --buyer <pk>                  (caller = buyer or seller)
 *   status  --id <n> --buyer <pk>
 */

import * as anchor from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  Connection,
} from "@solana/web3.js";
import { BN, Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import * as fs from "fs";
import * as path from "path";

// ─── Config ──────────────────────────────────────────────────────────────────

const CLUSTER = process.env.CLUSTER || "devnet";
const RPC_URL =
  process.env.RPC_URL ||
  (CLUSTER === "devnet"
    ? "https://api.devnet.solana.com"
    : "http://127.0.0.1:8899");

const PROGRAM_ID = new PublicKey(
  "9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF"
);

// Load IDL
const IDL = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../target/idl/solana_escrow_engine.json"),
    "utf-8"
  )
);

// ─── Helpers ─────────────────────────────────────────────────────────────────

function loadKeypair(pathOrEnv: string): Keypair {
  if (fs.existsSync(pathOrEnv)) {
    const raw = JSON.parse(fs.readFileSync(pathOrEnv, "utf-8"));
    return Keypair.fromSecretKey(Uint8Array.from(raw));
  }
  // default: solana config keypair
  const defaultPath = path.join(
    process.env.HOME || "~",
    ".config/solana/id.json"
  );
  const raw = JSON.parse(fs.readFileSync(defaultPath, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function getPDAs(buyer: PublicKey, escrowId: BN) {
  const idBytes = escrowId.toArrayLike(Buffer, "le", 8);
  const [escrowPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), buyer.toBuffer(), idBytes],
    PROGRAM_ID
  );
  const [vaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), buyer.toBuffer(), idBytes],
    PROGRAM_ID
  );
  return { escrowPda, vaultPda };
}

function stateLabel(state: any): string {
  if (state.active !== undefined) return "ACTIVE (awaiting deposit)";
  if (state.funded !== undefined) return "FUNDED (in escrow)";
  if (state.released !== undefined) return "RELEASED (seller paid)";
  if (state.refunded !== undefined) return "REFUNDED (buyer refunded)";
  if (state.disputed !== undefined) return "DISPUTED (arbiter required)";
  return JSON.stringify(state);
}

// ─── Commands ────────────────────────────────────────────────────────────────

async function cmdInit(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const sellerPk = new PublicKey(requireArg(args, "--seller"));
  const arbiterPk = new PublicKey(requireArg(args, "--arbiter"));
  const amountSol = parseFloat(requireArg(args, "--amount"));
  const description = requireArg(args, "--desc");
  const keypairPath = getArg(args, "--keypair") || "";

  const payer = loadKeypair(keypairPath);
  const { escrowPda, vaultPda } = getPDAs(payer.publicKey, escrowId);

  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  const amount = new BN(amountSol * LAMPORTS_PER_SOL);

  console.log(`\nInitializing escrow #${escrowId}...`);
  console.log(`  Buyer:   ${payer.publicKey.toBase58()}`);
  console.log(`  Seller:  ${sellerPk.toBase58()}`);
  console.log(`  Arbiter: ${arbiterPk.toBase58()}`);
  console.log(`  Amount:  ${amountSol} SOL (${amount} lamports)`);
  console.log(`  PDA:     ${escrowPda.toBase58()}`);
  console.log(`  Vault:   ${vaultPda.toBase58()}`);

  const tx = await (program.methods as any)
    .initializeEscrow(escrowId, sellerPk, arbiterPk, amount, description)
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([payer])
    .rpc();

  console.log(`\n✅ Success! Transaction: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);
  console.log(`   Escrow PDA: https://explorer.solana.com/address/${escrowPda.toBase58()}?cluster=${CLUSTER}`);
}

async function cmdDeposit(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const buyerPk = new PublicKey(requireArg(args, "--buyer"));
  const keypairPath = getArg(args, "--keypair") || "";
  const payer = loadKeypair(keypairPath);

  const { escrowPda, vaultPda } = getPDAs(buyerPk, escrowId);
  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  console.log(`\nDepositing funds into escrow #${escrowId}...`);

  const tx = await (program.methods as any)
    .deposit(escrowId)
    .accounts({
      buyer: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([payer])
    .rpc();

  console.log(`\n✅ Deposited! Transaction: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);
}

async function cmdRelease(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const buyerPk = new PublicKey(requireArg(args, "--buyer"));
  const sellerPk = new PublicKey(requireArg(args, "--seller"));
  const keypairPath = getArg(args, "--keypair") || "";
  const payer = loadKeypair(keypairPath);

  const { escrowPda, vaultPda } = getPDAs(buyerPk, escrowId);
  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  console.log(`\nReleasing escrow #${escrowId} to seller...`);

  const tx = await (program.methods as any)
    .release(escrowId)
    .accounts({
      caller: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      seller: sellerPk,
    })
    .signers([payer])
    .rpc();

  console.log(`\n✅ Released! Transaction: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);
}

async function cmdRefund(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const buyerPk = new PublicKey(requireArg(args, "--buyer"));
  const keypairPath = getArg(args, "--keypair") || "";
  const payer = loadKeypair(keypairPath);

  const { escrowPda, vaultPda } = getPDAs(buyerPk, escrowId);
  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  console.log(`\nRefunding escrow #${escrowId} to buyer...`);

  const tx = await (program.methods as any)
    .refund(escrowId)
    .accounts({
      caller: payer.publicKey,
      escrow: escrowPda,
      vault: vaultPda,
      buyer: buyerPk,
    })
    .signers([payer])
    .rpc();

  console.log(`\n✅ Refunded! Transaction: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);
}

async function cmdDispute(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const buyerPk = new PublicKey(requireArg(args, "--buyer"));
  const keypairPath = getArg(args, "--keypair") || "";
  const payer = loadKeypair(keypairPath);

  const { escrowPda } = getPDAs(buyerPk, escrowId);
  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  console.log(`\nOpening dispute on escrow #${escrowId}...`);

  const tx = await (program.methods as any)
    .dispute(escrowId)
    .accounts({
      caller: payer.publicKey,
      escrow: escrowPda,
    })
    .signers([payer])
    .rpc();

  console.log(`\n✅ Disputed! Transaction: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);
}

async function cmdStatus(args: string[]) {
  const escrowId = new BN(requireArg(args, "--id"));
  const buyerPk = new PublicKey(requireArg(args, "--buyer"));
  const keypairPath = getArg(args, "--keypair") || "";
  const payer = loadKeypair(keypairPath);

  const { escrowPda, vaultPda } = getPDAs(buyerPk, escrowId);
  const connection = new Connection(RPC_URL, "confirmed");
  const provider = buildProvider(payer);
  const program = new Program(IDL, provider);

  const escrow = await (program.account as any).escrowAccount.fetch(escrowPda);
  const vaultBalance = await connection.getBalance(vaultPda);

  console.log(`\n📋 Escrow #${escrowId} Status`);
  console.log(`  State:       ${stateLabel(escrow.state)}`);
  console.log(`  Buyer:       ${escrow.buyer.toBase58()}`);
  console.log(`  Seller:      ${escrow.seller.toBase58()}`);
  console.log(`  Arbiter:     ${escrow.arbiter.toBase58()}`);
  console.log(
    `  Amount:      ${escrow.amount.toNumber() / LAMPORTS_PER_SOL} SOL (${escrow.amount} lamports)`
  );
  console.log(
    `  Vault:       ${vaultBalance / LAMPORTS_PER_SOL} SOL held in PDA`
  );
  console.log(`  Description: ${escrow.description}`);
  console.log(
    `  Created at:  ${new Date(escrow.createdAt.toNumber() * 1000).toISOString()}`
  );
  console.log(
    `\n  🔍 View on Explorer: https://explorer.solana.com/address/${escrowPda.toBase58()}?cluster=${CLUSTER}`
  );
}

// ─── Utility ─────────────────────────────────────────────────────────────────

function buildProvider(signer: Keypair): AnchorProvider {
  const connection = new Connection(RPC_URL, "confirmed");
  const wallet = new Wallet(signer);
  return new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
}

function requireArg(args: string[], flag: string): string {
  const idx = args.indexOf(flag);
  if (idx === -1 || idx + 1 >= args.length) {
    console.error(`Missing required argument: ${flag}`);
    process.exit(1);
  }
  return args[idx + 1];
}

function getArg(args: string[], flag: string): string | undefined {
  const idx = args.indexOf(flag);
  if (idx === -1 || idx + 1 >= args.length) return undefined;
  return args[idx + 1];
}

// ─── Main ─────────────────────────────────────────────────────────────────────

const COMMANDS: Record<string, (args: string[]) => Promise<void>> = {
  init: cmdInit,
  deposit: cmdDeposit,
  release: cmdRelease,
  refund: cmdRefund,
  dispute: cmdDispute,
  status: cmdStatus,
};

const [, , cmd, ...rest] = process.argv;

if (!cmd || !COMMANDS[cmd]) {
  console.log(`
Solana Escrow Engine CLI
========================
Commands:
  init      --id <n> --seller <pk> --arbiter <pk> --amount <sol> --desc "<text>" [--keypair <path>]
  deposit   --id <n> --buyer <pk> [--keypair <path>]
  release   --id <n> --buyer <pk> --seller <pk> [--keypair <path>]
  refund    --id <n> --buyer <pk> [--keypair <path>]
  dispute   --id <n> --buyer <pk> [--keypair <path>]
  status    --id <n> --buyer <pk> [--keypair <path>]

Environment:
  CLUSTER   localnet | devnet (default: devnet)
  RPC_URL   custom RPC endpoint

Examples:
  # Create an escrow
  CLUSTER=devnet npx ts-node client/cli.ts init \\
    --id 42 \\
    --seller <SELLER_PUBKEY> \\
    --arbiter <ARBITER_PUBKEY> \\
    --amount 0.5 \\
    --desc "Payment for logo design"

  # Check status
  CLUSTER=devnet npx ts-node client/cli.ts status --id 42 --buyer <BUYER_PUBKEY>
`);
  process.exit(0);
}

COMMANDS[cmd](rest).catch((err) => {
  console.error("\n❌ Error:", err.message || err);
  process.exit(1);
});
