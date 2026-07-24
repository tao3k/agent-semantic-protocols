//! Pure v1 receipt verification for Rust member build scripts.
//!
//! This crate starts no processes and writes no files. Canonical receipt-path
//! resolution and source snapshot collection are separate read-only adapters.

mod command_materialization;
mod command_path;
mod member_lookup;
mod model;
mod receipt_verification;

pub use command_materialization::prepare_command;
pub use model::{VerificationInput, VerifiedMemberReceipt};
pub use receipt_verification::verify_receipt_bytes;
