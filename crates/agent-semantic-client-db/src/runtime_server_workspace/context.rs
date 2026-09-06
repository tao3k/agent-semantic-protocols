// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Resident workspace identity and lifecycle cancellation context.

use crate::runtime_generation_cancellation::GenerationCancellation;

/// Stable context shared by all work admitted for one resident workspace.
#[derive(Clone, Debug)]
pub struct WorkspaceRuntimeContext {
    workspace_identity: String,
    cancellation: GenerationCancellation,
}

impl WorkspaceRuntimeContext {
    pub(crate) fn new(workspace_identity: impl Into<String>) -> Self {
        Self {
            workspace_identity: workspace_identity.into(),
            cancellation: GenerationCancellation::new(),
        }
    }
    #[must_use]
    pub fn workspace_identity(&self) -> &str {
        &self.workspace_identity
    }
    #[must_use]
    pub fn cancellation(&self) -> GenerationCancellation {
        self.cancellation.clone()
    }
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}
