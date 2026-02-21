use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::EscrowError;
use crate::events::EscrowReleased;

/// Release escrowed funds to the seller.
///
/// ## Web2 Equivalent: `POST /api/escrows/:id/release`
///
/// In a traditional backend:
/// - Triggers bank disbursement from holding account → seller's bank account
/// - Can be called by buyer (happy path) or arbiter (dispute resolution)
/// - Processed by payment processor; takes 1-3 business days
/// - May involve ACH, wire, or card reverse
///
/// ## Solana Approach
///
/// - Directly modifies lamport balances: vault PDA → seller's wallet
/// - Settlement is **instant** (sub-second finality on Solana, ~400ms)
/// - No payment processor; on-chain code enforces who can call this
/// - Works in both `Funded` and `Disputed` states
/// - Atomic: either the full amount moves or nothing changes
pub fn handler(ctx: Context<Release>, escrow_id: u64) -> Result<()> {
    let escrow = &ctx.accounts.escrow;

    // ── State guard: must be Funded or Disputed ─────────────────────────
    require!(
        escrow.state == EscrowState::Funded || escrow.state == EscrowState::Disputed,
        EscrowError::InvalidState
    );

    // ── Authorization: only buyer or arbiter may release ────────────────
    // Web2: `if (req.user.role !== 'buyer' && req.user.role !== 'arbiter') return 403;`
    let caller = ctx.accounts.caller.key();
    require!(
        caller == escrow.buyer || caller == escrow.arbiter,
        EscrowError::Unauthorized
    );

    let amount = escrow.amount;
    let seller_key = escrow.seller;

    // ── Verify the seller account matches the escrow record ─────────────
    require!(
        ctx.accounts.seller.key() == seller_key,
        EscrowError::InvalidSeller
    );

    // ── Transfer lamports: vault PDA → seller wallet ────────────────────
    // Because the vault is owned by this program (Anchor `init`), the
    // program can directly modify lamports — no CPI needed.
    ctx.accounts.vault.sub_lamports(amount)?;
    ctx.accounts.seller.add_lamports(amount)?;

    // ── State transition: → Released ────────────────────────────────────
    let escrow = &mut ctx.accounts.escrow;
    escrow.state = EscrowState::Released;

    emit!(EscrowReleased {
        escrow_id,
        seller: seller_key,
        amount,
        released_by: caller,
    });

    msg!(
        "[EscrowEngine] Escrow #{} released | {} lamports → seller {} | by {}",
        escrow_id,
        amount,
        seller_key,
        caller
    );
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Release<'info> {
    /// The caller — must be buyer or arbiter.
    pub caller: Signer<'info>,

    /// The escrow account being released.
    #[account(
        mut,
        seeds = [ESCROW_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    /// The vault PDA holding the escrowed SOL.
    #[account(
        mut,
        seeds = [VAULT_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// CHECK: Seller's wallet — verified against `escrow.seller` in handler.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,
}
