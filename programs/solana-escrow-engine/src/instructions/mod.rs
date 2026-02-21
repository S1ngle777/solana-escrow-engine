pub mod initialize;
pub mod deposit;
pub mod release;
pub mod refund;
pub mod dispute;
pub mod close;

// Re-export everything from each instruction module so Anchor's generated
// code can find the __client_accounts_* and __cpi_client_accounts_* modules
// at the crate root level (via `use instructions::*` in lib.rs).
//
// The `handler` function names intentionally collide across modules — they are
// always called via their fully-qualified path (e.g., `instructions::deposit::handler`).
#[allow(ambiguous_glob_reexports)]
pub use initialize::*;
pub use deposit::*;
pub use release::*;
pub use refund::*;
pub use dispute::*;
pub use close::*;
