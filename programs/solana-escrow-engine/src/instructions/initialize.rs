use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::EscrowError;
use crate::events::EscrowCreated;

/// Create a new escrow between buyer, seller, and arbiter.
///
/// ## Web2 Equivalent: `POST /api/escrows`
///
/// In a traditional backend:
/// - Inserts a row into the `escrows` table in PostgreSQL
/// - Creates a holding account in Stripe/bank for the funds
/// - Returns a JSON response with the new `escrow_id`
///
/// ## Solana Approach
///
/// - Creates two Program Derived Addresses (PDAs) owned by this program:
///   1. **EscrowAccount** — stores all metadata (buyer, seller, arbiter, amount, state)
///   2. **VaultAccount** — a data-less PDA that will hold the escrowed SOL
/// - PDAs are deterministic: `seeds = ["escrow"/"vault", buyer_pubkey, escrow_id]`
/// - No central database — state lives on-chain, readable by anyone
/// - Account creation is atomic — either both PDAs are created or neither is
pub fn handler(
    ctx: Context<InitializeEscrow>,
    escrow_id: u64,
    seller: Pubkey,
    arbiter: Pubkey,
    amount: u64,
    description: String,
) -> Result<()> {
    // ── Validation (equivalent to Express.js middleware / Joi schema) ─────
    require!(
        description.len() <= MAX_DESCRIPTION,
        EscrowError::DescriptionTooLong
    );
    require!(amount > 0, EscrowError::InvalidAmount);
    require!(
        seller != ctx.accounts.buyer.key(),
        EscrowError::BuyerCannotBeSeller
    );
    require!(
        arbiter != ctx.accounts.buyer.key(),
        EscrowError::InvalidArbiter
    );
    require!(arbiter != seller, EscrowError::InvalidArbiter);

    // ── Write state (equivalent to INSERT INTO escrows ...) ──────────────
    let escrow = &mut ctx.accounts.escrow;
    escrow.escrow_id = escrow_id;
    escrow.buyer = ctx.accounts.buyer.key();
    escrow.seller = seller;
    escrow.arbiter = arbiter;
    escrow.amount = amount;
    escrow.description = description;
    escrow.state = EscrowState::Active;
    escrow.created_at = Clock::get()?.unix_timestamp;
    escrow.escrow_bump = ctx.bumps.escrow;
    escrow.vault_bump = ctx.bumps.vault;

    // ── Emit event (equivalent to publishing to webhook / message queue) ─
    emit!(EscrowCreated {
        escrow_id,
        buyer: ctx.accounts.buyer.key(),
        seller,
        arbiter,
        amount,
    });

    msg!(
        "[EscrowEngine] Escrow #{} created | buyer={} | seller={} | amount={} lamports",
        escrow_id,
        ctx.accounts.buyer.key(),
        seller,
        amount
    );
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct InitializeEscrow<'info> {
    /// The buyer who creates and funds the escrow. Pays rent for both PDAs.
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// EscrowAccount PDA — stores escrow metadata.
    /// Seeds: `["escrow", buyer, escrow_id]`
    #[account(
        init,
        payer = buyer,
        space = EscrowAccount::SIZE,
        seeds = [ESCROW_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump
    )]
    pub escrow: Account<'info, EscrowAccount>,

    /// VaultAccount PDA — will hold the escrowed SOL.
    /// Seeds: `["vault", buyer, escrow_id]`
    /// Owned by this program → only program logic can debit it.
    #[account(
        init,
        payer = buyer,
        space = VaultAccount::SIZE,
        seeds = [VAULT_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump
    )]
    pub vault: Account<'info, VaultAccount>,

    pub system_program: Program<'info, System>,
}
