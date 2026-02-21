use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::EscrowError;
use crate::events::EscrowRefunded;

/// Refund escrowed funds back to the buyer.
///
/// ## Web2 Equivalent: `POST /api/escrows/:id/refund`
///
/// In a traditional backend:
/// - Reversal from holding account back to buyer's bank account
/// - Initiated by seller (voluntarily) or arbiter (dispute resolution)
/// - Can take 5-10 business days; subject to bank fees
/// - Often involves ACH reversal or credit card chargeback processing
///
/// ## Solana Approach
///
/// - Lamports move instantly: vault PDA → buyer's wallet
/// - Seller can voluntarily refund; arbiter can override in disputes
/// - No bank intermediary, no processing fees beyond the tx cost (~$0.00025)
/// - Atomic: full refund or nothing — no partial or failed reversals
pub fn handler(ctx: Context<Refund>, escrow_id: u64) -> Result<()> {
    let escrow = &ctx.accounts.escrow;

    // ── State guard: must be Funded or Disputed ─────────────────────────
    require!(
        escrow.state == EscrowState::Funded || escrow.state == EscrowState::Disputed,
        EscrowError::InvalidState
    );

    // ── Authorization: only seller or arbiter may refund ────────────────
    let caller = ctx.accounts.caller.key();
    require!(
        caller == escrow.seller || caller == escrow.arbiter,
        EscrowError::Unauthorized
    );

    let amount = escrow.amount;
    let buyer_key = escrow.buyer;

    // ── Verify the buyer account matches the escrow record ──────────────
    require!(
        ctx.accounts.buyer.key() == buyer_key,
        EscrowError::InvalidBuyer
    );

    // ── Transfer lamports: vault PDA → buyer wallet ─────────────────────
    ctx.accounts.vault.sub_lamports(amount)?;
    ctx.accounts.buyer.add_lamports(amount)?;

    // ── State transition: → Refunded ────────────────────────────────────
    let escrow = &mut ctx.accounts.escrow;
    escrow.state = EscrowState::Refunded;

    emit!(EscrowRefunded {
        escrow_id,
        buyer: buyer_key,
        amount,
        refunded_by: caller,
    });

    msg!(
        "[EscrowEngine] Escrow #{} refunded | {} lamports → buyer {} | by {}",
        escrow_id,
        amount,
        buyer_key,
        caller
    );
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Refund<'info> {
    /// The caller — must be seller or arbiter.
    pub caller: Signer<'info>,

    /// The escrow account being refunded.
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

    /// CHECK: Buyer's wallet — verified against `escrow.buyer` in handler.
    #[account(mut)]
    pub buyer: UncheckedAccount<'info>,
}
