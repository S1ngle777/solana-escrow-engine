use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::EscrowError;
use crate::events::EscrowClosed;

/// Close a settled escrow and reclaim rent to the buyer.
///
/// ## Web2 Equivalent: Database Cleanup / Record Archival
///
/// In a traditional backend:
/// - After an escrow is completed or refunded, the record stays in the DB
/// - Eventually a cron job archives old records to cold storage or deletes them
/// - Storage cost is negligible per-row in SQL databases
///
/// ## Solana Approach
///
/// On Solana, **storage is expensive**: every account must maintain a minimum
/// lamport balance (rent). For our 335-byte `EscrowAccount`, that's ~0.003 SOL.
/// The 8-byte `VaultAccount` costs ~0.001 SOL. Combined: ~0.004 SOL per escrow.
///
/// `close_escrow` reclaims this rent by closing both PDA accounts and returning
/// the lamports to the original buyer (who paid rent at creation).
///
/// This is a critical Solana pattern: **clean up accounts you no longer need**.
/// Without it, a buyer who creates 100 escrows would permanently lose ~0.4 SOL
/// in rent — even after every escrow is settled.
///
/// ### Security
/// - Can only be called on `Released` or `Refunded` escrows (settled state)
/// - Only the buyer (who paid rent) can close
/// - Anchor's `close` constraint zeroes account data and transfers lamports
pub fn handler(ctx: Context<CloseEscrow>, escrow_id: u64) -> Result<()> {
    let escrow = &ctx.accounts.escrow;

    // ── State guard: only settled escrows can be closed ──────────────────
    require!(
        escrow.state == EscrowState::Released || escrow.state == EscrowState::Refunded,
        EscrowError::EscrowNotSettled
    );

    // ── Authorization: only the buyer (rent payer) can close ────────────
    require!(
        ctx.accounts.buyer.key() == escrow.buyer,
        EscrowError::Unauthorized
    );

    // Calculate rent being reclaimed (for event emission)
    let escrow_lamports = ctx.accounts.escrow.to_account_info().lamports();
    let vault_lamports = ctx.accounts.vault.to_account_info().lamports();
    let total_reclaimed = escrow_lamports + vault_lamports;

    emit!(EscrowClosed {
        escrow_id,
        rent_reclaimed: total_reclaimed,
        closed_by: ctx.accounts.buyer.key(),
    });

    msg!(
        "[EscrowEngine] Escrow #{} closed | {} lamports rent reclaimed by buyer {}",
        escrow_id,
        total_reclaimed,
        ctx.accounts.buyer.key()
    );

    // Note: Anchor `close = buyer` handles the actual account closure:
    //   1. Transfers all remaining lamports → buyer
    //   2. Zeros out account data
    //   3. Assigns account ownership to system program (effectively deletes it)
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct CloseEscrow<'info> {
    /// The buyer who originally created (and paid rent for) the escrow.
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// The escrow account to close. Rent returns to buyer.
    /// Anchor's `close = buyer` handles lamport transfer + data zeroing.
    #[account(
        mut,
        seeds = [ESCROW_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
        has_one = buyer @ EscrowError::Unauthorized,
        close = buyer,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    /// The vault PDA to close. Rent returns to buyer.
    #[account(
        mut,
        seeds = [VAULT_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
        close = buyer,
    )]
    pub vault: Account<'info, VaultAccount>,
}
