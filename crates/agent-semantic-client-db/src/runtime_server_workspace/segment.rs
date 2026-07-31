use super::model::{
    WORKSPACE_GENERATION_SCHEMA_ID, WorkspaceGenerationSnapshot, WorkspaceGenerationState,
    WorkspaceMemoryBackend, WorkspaceMemoryGeneration,
};
use super::pointer::WorkspaceGenerationPointerWriter;
use memmap2::{Mmap, MmapOptions};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs;

const SEGMENT_MAGIC: &[u8; 16] = b"ASPWSGENERATION1";
const SEGMENT_HEADER_LEN: usize = 64;

#[derive(Debug)]
pub struct MappedWorkspaceGeneration {
    mapping: Mmap,
    backend: Arc<WorkspaceMemoryBackend>,
}

impl MappedWorkspaceGeneration {
    pub async fn open(path: &Path) -> Result<Self, String> {
        let file = fs::File::open(path)
            .await
            .map_err(|error| format!("open workspace generation segment: {error}"))?;
        let file = file.into_std().await;
        let mapping = unsafe {
            // SAFETY: the mapped file is immutable after atomic publication and
            // the mapping lifetime is owned by this value.
            MmapOptions::new()
                .map(&file)
                .map_err(|error| format!("map workspace generation segment: {error}"))?
        };
        let generation = decode_segment(&mapping)?;
        let backend = Arc::new(WorkspaceMemoryBackend::from_generation(generation)?);
        Ok(Self { mapping, backend })
    }

    pub(crate) fn backend(&self) -> Arc<WorkspaceMemoryBackend> {
        Arc::clone(&self.backend)
    }

    pub fn mapped_len(&self) -> usize {
        self.mapping.len()
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceGenerationPublisher {
    directory: PathBuf,
    pointer: WorkspaceGenerationPointerWriter,
}

impl WorkspaceGenerationPublisher {
    pub async fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory)
            .await
            .map_err(|error| format!("create workspace generation directory: {error}"))?;
        let pointer = WorkspaceGenerationPointerWriter::open(&directory).await?;
        Ok(Self { directory, pointer })
    }

    pub async fn publish(
        &self,
        generation: &WorkspaceMemoryGeneration,
        previous_epoch_readable: bool,
    ) -> Result<(WorkspaceGenerationSnapshot, MappedWorkspaceGeneration), String> {
        generation.validate()?;
        let segment = encode_segment(generation)?;
        let final_path = self
            .directory
            .join(format!("generation-{}.mmap", generation.active_epoch));
        let temporary_path = self
            .directory
            .join(format!(".generation-{}.pending", generation.active_epoch));
        fs::write(&temporary_path, segment)
            .await
            .map_err(|error| format!("write workspace generation segment: {error}"))?;
        fs::rename(&temporary_path, &final_path)
            .await
            .map_err(|error| format!("publish workspace generation segment: {error}"))?;
        let mapped = MappedWorkspaceGeneration::open(&final_path).await?;
        let snapshot = WorkspaceGenerationSnapshot {
            schema_id: WORKSPACE_GENERATION_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            state: WorkspaceGenerationState::Ready,
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
            root_depth: generation.root_depth,
            memory_backend_digest: generation.memory_backend_digest.clone(),
            mmap_segment_path: final_path.to_string_lossy().into_owned(),
            previous_epoch_readable,
        };
        snapshot.validate()?;
        self.pointer.publish(&snapshot).await?;
        Ok((snapshot, mapped))
    }

    pub fn pointer_path(&self) -> &Path {
        self.pointer.path()
    }
}

fn encode_segment(generation: &WorkspaceMemoryGeneration) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(generation)
        .map_err(|error| format!("encode workspace generation segment: {error}"))?;
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "workspace generation segment is too large".to_owned())?;
    let digest = blake3::hash(&payload);
    let mut segment = Vec::with_capacity(SEGMENT_HEADER_LEN + payload.len());
    segment.extend_from_slice(SEGMENT_MAGIC);
    segment.extend_from_slice(&generation.active_epoch.to_le_bytes());
    segment.extend_from_slice(&payload_len.to_le_bytes());
    segment.extend_from_slice(digest.as_bytes());
    segment.extend_from_slice(&payload);
    Ok(segment)
}

fn decode_segment(mapping: &[u8]) -> Result<WorkspaceMemoryGeneration, String> {
    if mapping.len() < SEGMENT_HEADER_LEN || &mapping[..16] != SEGMENT_MAGIC {
        return Err("workspace generation segment header is invalid".to_owned());
    }
    let epoch = u64::from_le_bytes(
        mapping[16..24]
            .try_into()
            .map_err(|_| "workspace generation epoch is invalid".to_owned())?,
    );
    let payload_len = usize::try_from(u64::from_le_bytes(
        mapping[24..32]
            .try_into()
            .map_err(|_| "workspace generation length is invalid".to_owned())?,
    ))
    .map_err(|_| "workspace generation payload length overflows usize".to_owned())?;
    let end = SEGMENT_HEADER_LEN
        .checked_add(payload_len)
        .ok_or_else(|| "workspace generation payload length overflows".to_owned())?;
    if end != mapping.len() {
        return Err("workspace generation segment is truncated or has trailing bytes".to_owned());
    }
    let expected_digest = &mapping[32..64];
    let payload = &mapping[SEGMENT_HEADER_LEN..end];
    if blake3::hash(payload).as_bytes() != expected_digest {
        return Err("workspace generation segment digest mismatch".to_owned());
    }
    let generation: WorkspaceMemoryGeneration = serde_json::from_slice(payload)
        .map_err(|error| format!("decode workspace generation segment: {error}"))?;
    if generation.active_epoch != epoch {
        return Err("workspace generation segment epoch mismatch".to_owned());
    }
    generation.validate()?;
    Ok(generation)
}
