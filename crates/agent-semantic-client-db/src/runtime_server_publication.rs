//! Transport-neutral Ready generation publication channel.

use std::path::PathBuf;
use tokio::sync::watch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationPublished {
    pub workspace_identity: String,
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
