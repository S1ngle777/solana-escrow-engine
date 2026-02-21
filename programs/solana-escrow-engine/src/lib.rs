use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF");

// ──────────────────────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────────────────────

pub const ESCROW_SEED: &[u8] = b"escrow";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MAX_DESCRIPTION: usize = 200;

// ──────────────────────────────────────────────────────────────────────────────
// Program
// ──────────────────────────────────────────────────────────────────────────────

#[program]
pub mod solana_escrow_engine {
    use super::*;

    // ─── instruction: initialize_escrow ───────────────────────────────────────
    //
    // WEB2 EQUIVALENT:  POST /api/escrows
    //   - Inserts a row into the `escrows` table in a Postgres DB
    //   - Creates a holding account in Stripe/bank for the funds
    //   - Returns an escrow_id
    //
    // SOLANA APPROACH:
    //   - Creates two Program Derived Addresses (PDAs) owned by this program:
    //       1. `escrow` — stores all metadata (buyer, seller, arbiter, amount, state)
    //       2. `vault`  — a data-less PDA that will hold the escrowed SOL
    //   - PDAs are deterministic: seeds = ["escrow"/"vault", buyer_pubkey, escrow_id]
    //   - No central database. State lives ON-CHAIN, readable by anyone.
    //
    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        escrow_id: u64,
        seller: Pubkey,
        arbiter: Pubkey,
        amount: u64,
        description: String,
    ) -> Result<()> {
        require!(description.len() <= MAX_DESCRIPTION, EscrowError::DescriptionTooLong);
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

    // ─── instruction: deposit ─────────────────────────────────────────────────
    //
    // WEB2 EQUIVALENT:  PUT /api/escrows/:id/deposit
    //   - Initiates an ACH/wire transfer from buyer's bank into a holding account
    //   - Updates escrow DB record status to "funded"
    //   - A trusted third-party service (Stripe, Escrow.com) holds the funds
    //
    // SOLANA APPROACH:
    //   - Buyer signs a transaction that calls System Program via CPI
    //   - SOL moves from buyer's wallet → vault PDA (controlled by this program)
    //   - No third party: the program's logic IS the trust mechanism
    //   - Funds are cryptographically locked — only program logic can move them
    //
    pub fn deposit(ctx: Context<Deposit>, _escrow_id: u64) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        require!(escrow.state == EscrowState::Active, EscrowError::InvalidState);
        require!(
            ctx.accounts.buyer.key() == escrow.buyer,
            EscrowError::Unauthorized
        );

        let amount = escrow.amount;

        // CPI: System Program transfers SOL from buyer → vault PDA
        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        system_program::transfer(cpi_ctx, amount)?;

        escrow.state = EscrowState::Funded;

        msg!(
            "[EscrowEngine] Escrow #{} funded | {} lamports locked in vault",
            _escrow_id,
            amount
        );
        Ok(())
    }

    // ─── instruction: release ─────────────────────────────────────────────────
    //
    // WEB2 EQUIVALENT:  POST /api/escrows/:id/release
    //   - Triggers bank disbursement from holding account → seller's account
    //   - Can be called by buyer (happy path) or arbiter (dispute resolution)
    //   - Processed by payment processor, takes 1-3 business days
    //
    // SOLANA APPROACH:
    //   - Directly modifies lamport balances: vault PDA → seller's wallet
    //   - Settlement is INSTANT (sub-second finality on Solana)
    //   - No payment processor; on-chain code enforces who can call this
    //   - Works in both Funded and Disputed states
    //
    pub fn release(ctx: Context<Release>, escrow_id: u64) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        require!(
            escrow.state == EscrowState::Funded || escrow.state == EscrowState::Disputed,
            EscrowError::InvalidState
        );

        let caller = ctx.accounts.caller.key();
        require!(
            caller == escrow.buyer || caller == escrow.arbiter,
            EscrowError::Unauthorized
        );

        let amount = escrow.amount;
        let seller_key = escrow.seller;

        require!(
            ctx.accounts.seller.key() == seller_key,
            EscrowError::InvalidSeller
        );

        // Transfer lamports: vault PDA → seller wallet
        // Vault is owned by this program (Anchor init), so program can directly modify lamports
        ctx.accounts.vault.sub_lamports(amount)?;
        ctx.accounts.seller.add_lamports(amount)?;

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

    // ─── instruction: refund ──────────────────────────────────────────────────
    //
    // WEB2 EQUIVALENT:  POST /api/escrows/:id/refund
    //   - Reversal from holding account back to buyer's account
    //   - Initiated by seller (voluntarily) or arbiter (dispute resolution)
    //   - Can take 5-10 business days; subject to bank fees
    //
    // SOLANA APPROACH:
    //   - Lamports move instantly: vault → buyer wallet
    //   - Seller can voluntarily refund; arbiter can override in disputes
    //   - No bank intermediary, no fees beyond tx cost (~0.000005 SOL)
    //
    pub fn refund(ctx: Context<Refund>, escrow_id: u64) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        require!(
            escrow.state == EscrowState::Funded || escrow.state == EscrowState::Disputed,
            EscrowError::InvalidState
        );

        let caller = ctx.accounts.caller.key();
        require!(
            caller == escrow.seller || caller == escrow.arbiter,
            EscrowError::Unauthorized
        );

        let amount = escrow.amount;
        let buyer_key = escrow.buyer;

        require!(
            ctx.accounts.buyer.key() == buyer_key,
            EscrowError::InvalidBuyer
        );

        // Transfer lamports: vault PDA → buyer wallet
        ctx.accounts.vault.sub_lamports(amount)?;
        ctx.accounts.buyer.add_lamports(amount)?;

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

    // ─── instruction: dispute ─────────────────────────────────────────────────
    //
    // WEB2 EQUIVALENT:  POST /api/escrows/:id/dispute
    //   - Opens a support ticket / dispute case in the platform
    //   - Freezes funds in holding account pending resolution
    //   - Arbiter reviews evidence and decides outcome
    //
    // SOLANA APPROACH:
    //   - Sets on-chain state to Disputed — funds are still in vault PDA
    //   - Only buyer or seller can open dispute (on Funded escrow)
    //   - Arbiter's on-chain identity has exclusive authority to resolve
    //   - ALL state transitions are auditable via Solana blockchain explorer
    //
    pub fn dispute(ctx: Context<Dispute>, escrow_id: u64) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        require!(escrow.state == EscrowState::Funded, EscrowError::InvalidState);

        let caller = ctx.accounts.caller.key();
        require!(
            caller == escrow.buyer || caller == escrow.seller,
            EscrowError::Unauthorized
        );

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
}

// ──────────────────────────────────────────────────────────────────────────────
// Account Structs (State)
// ──────────────────────────────────────────────────────────────────────────────

/// On-chain state for a single escrow.
///
/// Web2 equivalent: a row in an `escrows` Postgres table:
///   id, buyer_id, seller_id, arbiter_id, amount, status, description, created_at
///
/// Solana model: PDA at seeds = ["escrow", buyer_pubkey, escrow_id_le_bytes]
/// Data is Borsh-serialized with 8-byte Anchor discriminator prefix.
#[account]
#[derive(Debug)]
pub struct EscrowAccount {
    pub escrow_id: u64,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub arbiter: Pubkey,
    pub amount: u64,
    pub description: String,
    pub state: EscrowState,
    pub created_at: i64,
    pub escrow_bump: u8,
    pub vault_bump: u8,
}

impl EscrowAccount {
    // discriminator(8) + escrow_id(8) + buyer(32) + seller(32) + arbiter(32)
    // + amount(8) + description(4 prefix + 200 bytes) + state(1) + created_at(8) + bumps(2)
    pub const SIZE: usize = 8 + 8 + 32 + 32 + 32 + 8 + (4 + MAX_DESCRIPTION) + 1 + 8 + 2;
}

/// Empty vault account — only holds SOL (lamports), no structured data.
///
/// Web2 equivalent: a holding bank account or Stripe balance that belongs to
/// neither buyer nor seller, controlled entirely by the escrow service.
///
/// Solana model: PDA at seeds = ["vault", buyer_pubkey, escrow_id_le_bytes]
/// Owned by this program — only our program logic can debit its lamports.
#[account]
pub struct VaultAccount {}

impl VaultAccount {
    pub const SIZE: usize = 8; // 8-byte anchor discriminator only
}

/// Lifecycle states of an escrow.
///
/// Web2 analogy:  PENDING → FUNDED → RELEASED / REFUNDED
///                                └── DISPUTED ──┘
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum EscrowState {
    /// Created but buyer hasn't deposited yet (Web2: "pending_payment")
    Active,
    /// Buyer deposited funds, awaiting release/refund (Web2: "in_escrow")
    Funded,
    /// Funds released to seller (Web2: "completed")
    Released,
    /// Funds returned to buyer (Web2: "refunded")
    Refunded,
    /// Dispute opened, arbiter must resolve (Web2: "disputed")
    Disputed,
}

// ──────────────────────────────────────────────────────────────────────────────
// Instruction Contexts (Account Validation)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init,
        payer = buyer,
        space = EscrowAccount::SIZE,
        seeds = [ESCROW_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump
    )]
    pub escrow: Account<'info, EscrowAccount>,

    /// Vault PDA — will hold the escrowed SOL.
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

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        seeds = [ESCROW_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
        has_one = buyer @ EscrowError::Unauthorized,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    #[account(
        mut,
        seeds = [VAULT_SEED, buyer.key().as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Release<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [ESCROW_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    #[account(
        mut,
        seeds = [VAULT_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// CHECK: Seller's wallet — verified against escrow.seller in instruction body
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Refund<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [ESCROW_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    #[account(
        mut,
        seeds = [VAULT_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// CHECK: Buyer's wallet — verified against escrow.buyer in instruction body
    #[account(mut)]
    pub buyer: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct Dispute<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [ESCROW_SEED, escrow.buyer.as_ref(), &escrow_id.to_le_bytes()],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Events
// ──────────────────────────────────────────────────────────────────────────────

#[event]
pub struct EscrowCreated {
    pub escrow_id: u64,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub arbiter: Pubkey,
    pub amount: u64,
}

#[event]
pub struct EscrowReleased {
    pub escrow_id: u64,
    pub seller: Pubkey,
    pub amount: u64,
    pub released_by: Pubkey,
}

#[event]
pub struct EscrowRefunded {
    pub escrow_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
    pub refunded_by: Pubkey,
}

#[event]
pub struct EscrowDisputed {
    pub escrow_id: u64,
    pub disputed_by: Pubkey,
}

// ──────────────────────────────────────────────────────────────────────────────
// Custom Errors
// ──────────────────────────────────────────────────────────────────────────────

#[error_code]
pub enum EscrowError {
    #[msg("Description exceeds 200 character limit")]
    DescriptionTooLong,
    #[msg("Escrow amount must be greater than zero")]
    InvalidAmount,
    #[msg("Buyer and seller must be different accounts")]
    BuyerCannotBeSeller,
    #[msg("Arbiter must be different from buyer and seller")]
    InvalidArbiter,
    #[msg("Caller is not authorized to perform this action")]
    Unauthorized,
    #[msg("Escrow is not in the required state for this operation")]
    InvalidState,
    #[msg("Provided seller account does not match escrow record")]
    InvalidSeller,
    #[msg("Provided buyer account does not match escrow record")]
    InvalidBuyer,
}
