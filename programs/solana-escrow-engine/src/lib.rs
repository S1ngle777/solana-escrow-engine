use anchor_lang::prelude::*;

declare_id!("9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF");

pub mod state;
pub mod errors;
pub mod events;
pub mod instructions;

use instructions::*;

// ──────────────────────────────────────────────────────────────────────────────
// Program
// ──────────────────────────────────────────────────────────────────────────────
//
// This program implements a trustless escrow engine on Solana, replacing the
// traditional Web2 stack of REST API + PostgreSQL + Stripe + server-side auth
// with a single Rust program that runs on a decentralized state machine.
//
// Architecture mapping:
//
// | Web2 Layer  | Solana Equivalent             |
// |-------------|-------------------------------|
// | REST API    | Program instructions          |
// | PostgreSQL  | PDA accounts (on-chain state) |
// | JWT/OAuth   | Ed25519 signature verification|
// | Stripe      | System program CPI            |
// | Trust layer | The program itself (math > middlemen) |
//
// ──────────────────────────────────────────────────────────────────────────────

#[program]
pub mod solana_escrow_engine {
    use super::*;

    /// Create a new escrow. **Web2**: `POST /api/escrows`
    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        escrow_id: u64,
        seller: Pubkey,
        arbiter: Pubkey,
        amount: u64,
        description: String,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, escrow_id, seller, arbiter, amount, description)
    }

    /// Fund an escrow with SOL. **Web2**: `PUT /api/escrows/:id/deposit`
    pub fn deposit(ctx: Context<Deposit>, escrow_id: u64) -> Result<()> {
        instructions::deposit::handler(ctx, escrow_id)
    }

    /// Release funds to the seller. **Web2**: `POST /api/escrows/:id/release`
    pub fn release(ctx: Context<Release>, escrow_id: u64) -> Result<()> {
        instructions::release::handler(ctx, escrow_id)
    }

    /// Refund funds to the buyer. **Web2**: `POST /api/escrows/:id/refund`
    pub fn refund(ctx: Context<Refund>, escrow_id: u64) -> Result<()> {
        instructions::refund::handler(ctx, escrow_id)
    }

    /// Open a dispute. **Web2**: `POST /api/escrows/:id/dispute`
    pub fn dispute(ctx: Context<Dispute>, escrow_id: u64) -> Result<()> {
        instructions::dispute::handler(ctx, escrow_id)
    }

    /// Close settled escrow & reclaim rent. **Web2**: `DELETE /api/escrows/:id`
    pub fn close_escrow(ctx: Context<CloseEscrow>, escrow_id: u64) -> Result<()> {
        instructions::close::handler(ctx, escrow_id)
    }
}
