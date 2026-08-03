use std::path::Path;

use super::WorkspaceGenerationSnapshot;
use super::atomic_snapshot_pointer::{AtomicSnapshotPointerReader, AtomicSnapshotPointerWriter};

const POINTER_FILE_NAME: &str = "active-generation.pointer";
const POINTER_CONTEXT: &str = "workspace generation";

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_pointer.rs"]
mod tests;

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceGenerationPointerWriter {
    inner: AtomicSnapshotPointerWriter,
}

impl WorkspaceGenerationPointerWriter {
    pub(crate) async fn open(directory: &Path) -> Result<Self, String> {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|error| {
                format!(
                    "failed to create workspace generation directory `{}`: {error}",
                    directory.display()
                )
            })?;
        Ok(Self {
            inner: AtomicSnapshotPointerWriter::open(
                directory.join(POINTER_FILE_NAME),
                POINTER_CONTEXT,
            )
            .await?,
        })
    }

    pub(crate) async fn publish(
        &self,
        snapshot: &WorkspaceGenerationSnapshot,
    ) -> Result<(), String> {
        self.inner.publish(snapshot).await
    }

    pub(crate) fn path(&self) -> &Path {
        self.inner.path()
    }
}

#[derive(Debug)]
pub struct WorkspaceGenerationPointerReader {
    inner: AtomicSnapshotPointerReader<WorkspaceGenerationSnapshot>,
}

impl WorkspaceGenerationPointerReader {
    pub async fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            inner: AtomicSnapshotPointerReader::open(path, POINTER_CONTEXT).await?,
        })
    }

    pub fn read(&self) -> Result<WorkspaceGenerationSnapshot, String> {
        self.inner.read()
    }

    pub(crate) fn read_optional(&self) -> Result<Option<WorkspaceGenerationSnapshot>, String> {
        self.inner.read_optional()
    }
}
