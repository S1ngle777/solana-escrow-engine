use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::*;
use crate::errors::EscrowError;

/// Fund an escrow by transferring SOL from the buyer to the vault PDA.
///
/// ## Web2 Equivalent: `PUT /api/escrows/:id/deposit`
///
/// In a traditional backend:
/// - Initiates an ACH/wire transfer from buyer's bank into a holding account
/// - Updates the escrow DB record status from `pending_payment` to `in_escrow`
/// - A trusted third-party service (Stripe, Escrow.com) holds the funds
///
/// ## Solana Approach
///
/// - Buyer signs a transaction that invokes `system_program::transfer` via CPI
/// - SOL moves from buyer's wallet → vault PDA (controlled by this program)
/// - No third party needed: the program's logic IS the trust mechanism
/// - Funds are cryptographically locked — only this program's release/refund
///   instructions can move them out of the vault
pub fn handler(ctx: Context<Deposit>, _escrow_id: u64) -> Result<()> {
    let escrow = &mut ctx.accounts.escrow;

    // ── State guard: only Active escrows can be funded ───────────────────
    require!(escrow.state == EscrowState::Active, EscrowError::InvalidState);
    require!(
        ctx.accounts.buyer.key() == escrow.buyer,
        EscrowError::Unauthorized
    );

    let amount = escrow.amount;

    // ── CPI: System Program transfers SOL from buyer → vault PDA ────────
    // Web2 equivalent: Stripe.charges.create({ amount, source: buyer })
    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.buyer.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        },
    );
    system_program::transfer(cpi_ctx, amount)?;

    // ── State transition: Active → Funded ────────────────────────────────
    escrow.state = EscrowState::Funded;

    msg!(
        "[EscrowEngine] Escrow #{} funded | {} lamports locked in vault",
        _escrow_id,
        amount
    );
    Ok(())
}

// ─── Account Context ─────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Deposit<'info> {
    /// The buyer who is depositing funds.
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// The escrow account to be funded.
    #[account(
        mut,
        seeds = [ESCROW_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
        has_one = buyer @ EscrowError::Unauthorized,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    /// The vault PDA that will receive and hold the SOL.
    #[account(
        mut,
        seeds = [VAULT_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    pub system_program: Program<'info, System>,
}
