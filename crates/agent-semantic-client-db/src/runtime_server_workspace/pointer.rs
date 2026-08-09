use std::path::Path;

use super::atomic_snapshot_pointer::{AtomicSnapshotPointerReader, AtomicSnapshotPointerWriter};
use super::{WorkspaceGenerationSnapshot, WorkspaceMemoryGeneration};

const POINTER_FILE_NAME: &str = "active-generation.pointer";
const POINTER_CONTEXT: &str = "workspace generation";

pub(crate) const ACTIVE_WORKSPACE_GENERATION_REQUIRED: &str = "state=source-unavailable reasonKind=active-workspace-generation-required: active workspace generation lease is required";

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
    pub(crate) async fn matches_generation(
        path: &Path,
        generation: &WorkspaceMemoryGeneration,
    ) -> bool {
        let Ok(reader) = Self::open(path).await else {
            return false;
        };
        let Ok(snapshot) = reader.read() else {
            return false;
        };
        snapshot.validate().is_ok()
            && snapshot.workspace_identity == generation.workspace_identity
            && snapshot.generation_digest == generation.generation_digest
    }

    pub async fn open(path: &Path) -> Result<Self, String> {
        Self::open_optional(path)
            .await?
            .ok_or_else(|| ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned())
    }

    pub async fn open_optional(path: &Path) -> Result<Option<Self>, String> {
        Ok(
            match AtomicSnapshotPointerReader::open_optional(path, POINTER_CONTEXT).await? {
                Some(inner) => Some(Self { inner }),
                None => None,
            },
        )
    }

    pub fn read(&self) -> Result<WorkspaceGenerationSnapshot, String> {
        self.inner.read()
    }

    pub(crate) fn committed_generation(&self) -> Option<u64> {
        self.inner.committed_generation()
    }

    pub(crate) fn read_previous_valid_optional(&self) -> Option<WorkspaceGenerationSnapshot> {
        self.inner.read_previous_valid_optional()
    }
}
