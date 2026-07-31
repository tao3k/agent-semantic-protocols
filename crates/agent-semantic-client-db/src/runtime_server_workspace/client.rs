use super::{
    lease::WorkspaceGenerationLease, model::WorkspaceMemoryBackend,
    pointer::WorkspaceGenerationPointerReader, segment::MappedWorkspaceGeneration,
};
use parking_lot::RwLock;
use std::{path::Path, sync::Arc};

#[derive(Debug)]
pub struct WorkspaceGenerationDataPlaneClient {
    pointer: WorkspaceGenerationPointerReader,
    current: RwLock<Arc<WorkspaceMemoryBackend>>,
}

#[derive(Debug)]
pub enum WorkspaceGenerationDataPlaneOpen {
    Ready(WorkspaceGenerationDataPlaneClient),
    Missing,
    RecoveryRequired { reason: String },
}

impl WorkspaceGenerationDataPlaneClient {
    pub async fn open_state(
        pointer_path: &Path,
    ) -> Result<WorkspaceGenerationDataPlaneOpen, String> {
        if !tokio::fs::try_exists(pointer_path)
            .await
            .map_err(|error| format!("inspect workspace generation pointer: {error}"))?
        {
            return Ok(WorkspaceGenerationDataPlaneOpen::Missing);
        }
        match Self::open(pointer_path).await {
            Ok(client) => Ok(WorkspaceGenerationDataPlaneOpen::Ready(client)),
            Err(reason) => Ok(WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason }),
        }
    }

    pub async fn open(pointer_path: &Path) -> Result<Self, String> {
        let pointer = WorkspaceGenerationPointerReader::open(pointer_path).await?;
        let snapshot = pointer.read()?;
        let mapped =
            MappedWorkspaceGeneration::open(Path::new(&snapshot.mmap_segment_path)).await?;
        let backend = mapped.backend();
        validate_pointer_generation(&snapshot, &backend)?;
        Ok(Self {
            pointer,
            current: RwLock::new(backend),
        })
    }

    pub fn lease(&self) -> WorkspaceGenerationLease {
        WorkspaceGenerationLease::from_backend(Arc::clone(&self.current.read()))
    }

    pub async fn refresh_if_changed(&self) -> Result<bool, String> {
        let snapshot = self.pointer.read()?;
        if self.current.read().generation().active_epoch == snapshot.active_epoch {
            return Ok(false);
        }
        let mapped =
            MappedWorkspaceGeneration::open(Path::new(&snapshot.mmap_segment_path)).await?;
        let backend = mapped.backend();
        validate_pointer_generation(&snapshot, &backend)?;
        *self.current.write() = backend;
        Ok(true)
    }
}

fn validate_pointer_generation(
    snapshot: &super::model::WorkspaceGenerationSnapshot,
    backend: &WorkspaceMemoryBackend,
) -> Result<(), String> {
    let generation = backend.generation();
    if generation.workspace_identity != snapshot.workspace_identity
        || generation.active_epoch != snapshot.active_epoch
        || generation.generation_digest != snapshot.generation_digest
        || generation.memory_backend_digest != snapshot.memory_backend_digest
        || generation.root_depth != snapshot.root_depth
    {
        return Err(format!(
            "workspace generation pointer does not match mapped generation: workspaceIdentity={} epoch={}",
            snapshot.workspace_identity, snapshot.active_epoch
        ));
    }
    Ok(())
}
