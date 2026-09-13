// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident workspace registry interface and owned implementation leaves.

pub(super) mod canonical_durability;
pub(super) mod canonical_publication;
mod core;
mod durability;
pub(super) mod owner_identity;
mod publication;
mod readiness;
mod runtime_projection_reads;
mod sparse_provider_owner_cache;
pub(super) mod writer_publication;

pub(super) use canonical_publication as canonical_publication_owner;
pub(super) use owner_identity as owner_identity_owner;
pub(super) use writer_publication as writer_publication_owner;

pub use core::RuntimeServerWorkspaceRegistry;
use core::WorkspaceWriteCommand;
pub use readiness::PublishedWorkspaceGenerationState;
