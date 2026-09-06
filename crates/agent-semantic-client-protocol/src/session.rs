// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! In-memory state machine for one Runtime client session.

use crate::ClientProjectId;
use crate::ClientSessionId;
use crate::ClientWorkspaceIdentity;

/// Lifecycle phase of a Runtime client session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSessionState {
    /// Allocated locally but not initialized by Runtime.
    Created,
    /// Bound to admitted session, project, and workspace identities.
    Initialized,
    /// Shutdown has been requested but the client has not exited.
    Shutdown,
    /// The client has left the session.
    Exited,
}

/// Runtime client session state and its admitted identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSession {
    pub(crate) state: ClientSessionState,
    pub(crate) session_id: Option<ClientSessionId>,
    pub(crate) project_id: Option<ClientProjectId>,
    pub(crate) workspace_id: Option<ClientWorkspaceIdentity>,
}
