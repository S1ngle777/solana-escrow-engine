use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::EscrowError;
use crate::events::EscrowDisputed;

/// Open a dispute on a funded escrow.
///
/// ## Web2 Equivalent: `POST /api/escrows/:id/dispute`
///
/// In a traditional backend:
/// - Opens a support ticket / dispute case in the platform's CRM
/// - Freezes funds in the holding account pending human review
/// - Arbiter (support team) reviews evidence and decides outcome
/// - Process can take days or weeks
///
/// ## Solana Approach
///
/// - Sets on-chain state to `Disputed` — funds stay locked in vault PDA
/// - Only buyer or seller can open a dispute (on a `Funded` escrow)
/// - Arbiter's on-chain identity (pubkey) has exclusive authority to resolve
/// - All state transitions are fully auditable via Solana blockchain explorer
/// - Resolution (release or refund) can happen in seconds, not days
pub fn handler(ctx: Context<Dispute>, escrow_id: u64) -> Result<()> {
    let escrow = &mut ctx.accounts.escrow;

    // ── State guard: disputes can only be opened on Funded escrows ───────
    require!(escrow.state == EscrowState::Funded, EscrowError::InvalidState);

    // ── Authorization: only buyer or seller may open dispute ────────────
    let caller = ctx.accounts.caller.key();
    require!(
        caller == escrow.buyer || caller == escrow.seller,
        EscrowError::Unauthorized
    );

    // ── State transition: Funded → Disputed ─────────────────────────────
    escrow.state = EscrowState::Disputed;

    emit!(EscrowDisputed {
        escrow_id,
        disputed_by: caller,
    });

    msg!(
        "[EscrowEngine] Escrow #{} disputed by {}",
        escrow_id,
        caller
    );
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Dispute<'info> {
    /// The caller — must be buyer or seller.
    pub caller: Signer<'info>,

    /// The escrow account entering dispute state.
    #[account(
        mut,
        seeds = [ESCROW_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,
}
