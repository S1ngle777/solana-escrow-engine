use anchor_lang::prelude::*;

// ──────────────────────────────────────────────────────────────────────────────
// Seeds & Limits
// ──────────────────────────────────────────────────────────────────────────────

pub const ESCROW_SEED: &[u8] = b"escrow";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MAX_DESCRIPTION: usize = 200;

// ──────────────────────────────────────────────────────────────────────────────
// EscrowAccount
// ──────────────────────────────────────────────────────────────────────────────

/// On-chain state for a single escrow.
///
/// **Web2 equivalent** — a row in a PostgreSQL `escrows` table:
///
/// ```sql
/// CREATE TABLE escrows (
///     id          BIGINT PRIMARY KEY,
///     buyer_id    UUID REFERENCES users(id),
///     seller_id   UUID REFERENCES users(id),
///     arbiter_id  UUID REFERENCES users(id),
///     amount      BIGINT NOT NULL,
///     status      VARCHAR(20) DEFAULT 'active',
///     description TEXT CHECK(LENGTH(description) <= 200),
///     created_at  TIMESTAMP DEFAULT NOW()
/// );
/// ```
///
/// **Solana model** — PDA at seeds `["escrow", buyer_pubkey, escrow_id_le_bytes]`.
/// Data is Borsh-serialized with an 8-byte Anchor discriminator prefix.
#[account]
#[derive(Debug)]
pub struct EscrowAccount {
    /// Unique identifier chosen by the buyer (analogous to a DB primary key).
    pub escrow_id: u64,
    /// The party depositing funds (like `buyer_id` foreign key).
    pub buyer: Pubkey,
    /// The party receiving payment on completion (like `seller_id`).
    pub seller: Pubkey,
    /// Neutral third party for dispute resolution (like `arbiter_id`).
    pub arbiter: Pubkey,
    /// Amount in lamports to be held in escrow.
    pub amount: u64,
    /// Human-readable description of the agreement (max 200 chars).
    pub description: String,
    /// Current lifecycle state of the escrow.
    pub state: EscrowState,
    /// Unix timestamp of creation (`created_at` column).
    pub created_at: i64,
    /// PDA bump for the escrow account.
    pub escrow_bump: u8,
    /// PDA bump for the vault account.
    pub vault_bump: u8,
}

impl EscrowAccount {
    /// Total on-chain space (bytes):
    ///
    /// | Field          | Size |
    /// |----------------|------|
    /// | discriminator  | 8    |
    /// | escrow_id      | 8    |
    /// | buyer          | 32   |
    /// | seller         | 32   |
    /// | arbiter        | 32   |
    /// | amount         | 8    |
    /// | description    | 4+200|
    /// | state          | 1    |
    /// | created_at     | 8    |
    /// | escrow_bump    | 1    |
    /// | vault_bump     | 1    |
    /// | **Total**      | **335** |
    pub const SIZE: usize = 8 + 8 + 32 + 32 + 32 + 8 + (4 + MAX_DESCRIPTION) + 1 + 8 + 2;
}

// ──────────────────────────────────────────────────────────────────────────────
// VaultAccount
// ──────────────────────────────────────────────────────────────────────────────

/// Empty vault account — only holds SOL (lamports), no structured data.
///
/// **Web2 equivalent** — a holding bank account or Stripe balance controlled
/// entirely by the escrow service. Neither buyer nor seller has direct access;
/// only the escrow service's business logic can move its funds.
///
/// **Solana model** — PDA at seeds `["vault", buyer_pubkey, escrow_id_le_bytes]`.
/// Owned by this program — only our instruction logic can debit its lamports.
#[account]
pub struct VaultAccount {}

impl VaultAccount {
    /// Only the 8-byte Anchor discriminator; all value is stored in lamports.
    pub const SIZE: usize = 8;
}

// ──────────────────────────────────────────────────────────────────────────────
// EscrowState
// ──────────────────────────────────────────────────────────────────────────────

/// Lifecycle states of an escrow, modeled as a finite state machine.
///
/// ```text
///   Active ──deposit()──► Funded ──release()──► Released
///                           │                     ▲
///                           ├──refund()──► Refunded│
///                           │                     │
///                           └──dispute()──► Disputed
///                                    └──release()/refund()──┘
/// ```
///
/// **Web2 analogy:**
///
/// | On-chain state | Web2 status      | Meaning                     |
/// |----------------|------------------|-----------------------------|
/// | `Active`       | `pending_payment`| Created, awaiting deposit   |
/// | `Funded`       | `in_escrow`      | Funds locked in vault       |
/// | `Released`     | `completed`      | Seller paid out             |
/// | `Refunded`     | `refunded`       | Buyer got money back        |
/// | `Disputed`     | `under_review`   | Arbiter must resolve        |
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum EscrowState {
    /// Created but buyer hasn't deposited yet.
    Active,
    /// Buyer deposited; funds locked in vault PDA.
    Funded,
    /// Funds released to seller.
    Released,
    /// Funds returned to buyer.
    Refunded,
    /// Dispute opened; arbiter must resolve.
    Disputed,
}
