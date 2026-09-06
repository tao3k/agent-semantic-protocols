// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

/// Typed state of the immutable resident generation currently published for a
/// workspace scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedWorkspaceGenerationState {
    Ready,
    Missing,
    RecoveryRequired { reason: String },
}
