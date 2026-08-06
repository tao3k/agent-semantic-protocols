//! Resident workspace registry interface and owned implementation leaves.

mod canonical_durability;
mod core;
mod durability;
mod owner_identity;
mod publication;
mod readiness;
mod runtime_projection_reads;
mod writer_publication;

pub use core::RuntimeServerWorkspaceRegistry;
use core::WorkspaceWriteCommand;
pub use readiness::PublishedWorkspaceGenerationState;
