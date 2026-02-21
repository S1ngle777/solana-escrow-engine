use anchor_lang::prelude::*;

/// Emitted when a new escrow is created.
///
/// **Web2 equivalent**: `escrow.created` webhook / event-bus message.
#[event]
pub struct EscrowCreated {
    pub escrow_id: u64,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub arbiter: Pubkey,
    pub amount: u64,
}

/// Emitted when escrowed funds are released to the seller.
///
/// **Web2 equivalent**: `payment.completed` webhook.
#[event]
pub struct EscrowReleased {
    pub escrow_id: u64,
    pub seller: Pubkey,
    pub amount: u64,
    pub released_by: Pubkey,
}

/// Emitted when escrowed funds are refunded to the buyer.
///
/// **Web2 equivalent**: `payment.refunded` webhook.
#[event]
pub struct EscrowRefunded {
    pub escrow_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
    pub refunded_by: Pubkey,
}

/// Emitted when a dispute is opened on a funded escrow.
///
/// **Web2 equivalent**: `dispute.opened` webhook.
#[event]
pub struct EscrowDisputed {
    pub escrow_id: u64,
    pub disputed_by: Pubkey,
}

/// Emitted when escrow accounts are closed and rent is reclaimed.
///
/// **Web2 equivalent**: `escrow.archived` — database record cleanup,
/// storage deallocation after retention period.
#[event]
pub struct EscrowClosed {
    pub escrow_id: u64,
    pub rent_reclaimed: u64,
    pub closed_by: Pubkey,
}
