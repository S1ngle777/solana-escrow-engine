use anchor_lang::prelude::*;

/// Custom error codes for the Escrow Engine program.
///
/// Each variant maps to a specific validation failure, analogous to HTTP
/// status codes in a REST API:
///
/// | Error                | HTTP Equivalent   | When                              |
/// |----------------------|-------------------|-----------------------------------|
/// | `DescriptionTooLong` | 400 Bad Request   | Description > 200 chars           |
/// | `InvalidAmount`      | 400 Bad Request   | Amount is zero                    |
/// | `BuyerCannotBeSeller`| 400 Bad Request   | Same pubkey for buyer & seller    |
/// | `InvalidArbiter`     | 400 Bad Request   | Arbiter == buyer or seller        |
/// | `Unauthorized`       | 403 Forbidden     | Caller lacks permission           |
/// | `InvalidState`       | 409 Conflict      | Wrong lifecycle state for action  |
/// | `InvalidSeller`      | 400 Bad Request   | Seller pubkey mismatch            |
/// | `InvalidBuyer`       | 400 Bad Request   | Buyer pubkey mismatch             |
/// | `EscrowNotSettled`   | 409 Conflict      | Close called on unsettled escrow  |
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

    #[msg("Escrow must be in Released or Refunded state to close")]
    EscrowNotSettled,
}
