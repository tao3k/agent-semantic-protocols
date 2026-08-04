//! Resident workspace registry interface and owned implementation leaves.

mod core;
mod durability;
mod owner_identity;
mod publication;
mod readiness;
mod search_authority;
mod writer_publication;

pub use core::RuntimeServerWorkspaceRegistry;
use core::WorkspaceWriteCommand;
pub use readiness::PublishedWorkspaceGenerationState;
