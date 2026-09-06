// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Transport-neutral Ready generation publication channel.

use agent_semantic_client_protocol::{ClientProjectId, ClientWorkspaceIdentity};
use std::path::PathBuf;
use tokio::sync::watch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationPublished {
    pub project_id: ClientProjectId,
    pub workspace_id: ClientWorkspaceIdentity,
    pub project_root: PathBuf,
    pub resident_pointer_path: PathBuf,
    pub generation_digest: String,
}

#[derive(Clone)]
pub struct WorkspaceGenerationPublication {
    sender: watch::Sender<Option<WorkspaceGenerationPublished>>,
}

impl WorkspaceGenerationPublication {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(None);
        Self { sender }
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<WorkspaceGenerationPublished>> {
        self.sender.subscribe()
    }

    pub fn publish(&self, value: WorkspaceGenerationPublished) {
        self.sender.send_replace(Some(value));
    }

    pub fn clear(&self) {
        self.sender.send_replace(None);
    }
}
