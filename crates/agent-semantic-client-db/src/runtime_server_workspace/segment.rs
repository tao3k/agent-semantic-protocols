use super::WorkspaceMemoryBackend;
use super::model::{
    WORKSPACE_GENERATION_SCHEMA_ID, WorkspaceGenerationSnapshot, WorkspaceGenerationState,
    WorkspaceMemoryGeneration,
};
use super::pointer::{WorkspaceGenerationPointerReader, WorkspaceGenerationPointerWriter};
use memmap2::{Mmap, MmapOptions};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs;
use tokio::io::AsyncWriteExt;

const SEGMENT_MAGIC: &[u8; 16] =
    agent_semantic_content_identity::workspace_memory_generation_segment::WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC;
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
        tokio::task::spawn_blocking(move || {
            let mapping = unsafe {
                // SAFETY: the mapped file is immutable after atomic publication and
                // the mapping lifetime is owned by this value.
                MmapOptions::new()
                    .map(&file)
                    .map_err(|error| format!("map workspace generation segment: {error}"))?
            };
            let generation = decode_segment(&mapping)?;
            let backend = Arc::new(WorkspaceMemoryBackend::from_validated_generation(
                generation,
            )?);
            Ok(Self { mapping, backend })
        })
        .await
        .map_err(|error| format!("map workspace generation task failed: {error}"))?
    }

    pub(crate) fn backend(&self) -> Arc<WorkspaceMemoryBackend> {
        Arc::clone(&self.backend)
    }

    pub fn mapped_len(&self) -> usize {
        self.mapping.len()
    }
}

#[derive(Debug)]
pub struct WorkspaceGenerationPublisher {
    directory: PathBuf,
    pointer_path: PathBuf,
    state: tokio::sync::OnceCell<WorkspaceGenerationPublisherState>,
}

#[derive(Debug)]
struct WorkspaceGenerationPublisherState {
    pointer: WorkspaceGenerationPointerWriter,
    owner_identity_journal: super::owner_identity_journal::RuntimeOwnerIdentityJournalPublisher,
    active_snapshot: tokio::sync::Mutex<Option<WorkspaceGenerationSnapshot>>,
}

impl WorkspaceGenerationPublisher {
    pub(super) async fn publish_owner_identity_delta(
        &self,
        workspace_identity: &str,
        base_generation_digest: &str,
        source_mutation_id: &str,
        delta: Vec<super::owner_identity_journal::RuntimeOwnerIdentityEntry>,
    ) -> Result<(), String> {
        self.state()
            .await?
            .owner_identity_journal
            .publish_delta(
                workspace_identity,
                base_generation_digest,
                source_mutation_id,
                delta,
            )
            .await
    }

    pub async fn new(directory: PathBuf) -> Result<Self, String> {
        let pointer_path = directory.join("active-generation.pointer");
        Ok(Self {
            directory,
            pointer_path,
            state: tokio::sync::OnceCell::new(),
        })
    }

    async fn state(&self) -> Result<&WorkspaceGenerationPublisherState, String> {
        self.state
            .get_or_try_init(|| async {
                fs::create_dir_all(&self.directory)
                    .await
                    .map_err(|error| format!("create workspace generation directory: {error}"))?;
                let pointer = WorkspaceGenerationPointerWriter::open(&self.directory).await?;
                let owner_identity_journal =
                    super::owner_identity_journal::RuntimeOwnerIdentityJournalPublisher::open(
                        self.directory.clone(),
                    )
                    .await?;
                let pointer_reader = WorkspaceGenerationPointerReader::open(pointer.path()).await?;
                let active = tokio::task::spawn_blocking(move || {
                    pointer_reader.read_previous_valid_optional()
                })
                .await
                .map_err(|error| {
                    format!("read workspace generation pointer task failed: {error}")
                })?;
                if let Some(active) = &active {
                    active.validate()?;
                    owner_identity_journal
                        .rebase(&active.workspace_identity, &active.generation_digest)
                        .await?;
                }
                Ok(WorkspaceGenerationPublisherState {
                    pointer,
                    owner_identity_journal,
                    active_snapshot: tokio::sync::Mutex::new(active),
                })
            })
            .await
    }

    pub async fn publish(
        &self,
        generation: std::sync::Arc<WorkspaceMemoryGeneration>,
        previous_epoch_readable: bool,
    ) -> Result<WorkspaceGenerationSnapshot, String> {
        let state = self.state().await?;
        // Publication is a single-writer transition. Besides preventing pointer races, this
        // lock lets retention distinguish the currently readable generation from abandoned
        // or superseded segment files without guessing from epoch numbers.
        let mut active_snapshot = state.active_snapshot.lock().await;
        let retention_started = tokio::time::Instant::now();
        prune_obsolete_generation_segments(
            &self.directory,
            active_snapshot
                .as_ref()
                .map(|snapshot| Path::new(&snapshot.mmap_segment_path)),
        )
        .await?;
        record_generation_stage(
            "generation-retention",
            &generation,
            retention_started.elapsed(),
            None,
        );
        let generation_encode_started = tokio::time::Instant::now();
        let (generation, segment, exact_segment, durable_commit_digest) =
            tokio::task::spawn_blocking(move || {
                generation.validate()?;
                let segment = encode_segment(&generation)?;
                let exact_segment =
                    super::exact_segment::encode_exact_projection_segment(&generation)?;
                let segment_digest =
                    agent_semantic_content_identity::ArtifactHash::blake3(&segment).value;
                let exact_segment_digest =
                    agent_semantic_content_identity::ArtifactHash::blake3(&exact_segment).value;
                let durable_commit_binding = format!(
                    "{}\u{1f}{}\u{1f}{}",
                    generation.generation_digest, segment_digest, exact_segment_digest
                );
                let durable_commit_digest = format!(
                    "blake3-256:{}",
                    agent_semantic_content_identity::ArtifactHash::blake3(
                        durable_commit_binding.as_bytes(),
                    )
                    .value,
                );
                Ok::<_, String>((generation, segment, exact_segment, durable_commit_digest))
            })
            .await
            .map_err(|error| format!("workspace generation encoder task failed: {error}"))??;
        record_generation_stage(
            "generation-segment-encode",
            &generation,
            generation_encode_started.elapsed(),
            u64::try_from(segment.len()).ok(),
        );
        let durable_publish_started = tokio::time::Instant::now();
        let final_path = self
            .directory
            .join(format!("generation-{}.mmap", generation.active_epoch));
        let exact_path = super::exact_segment::exact_projection_segment_path(&final_path);
        let temporary_path = self
            .directory
            .join(format!(".generation-{}.pending", generation.active_epoch));
        let exact_temporary_path = self.directory.join(format!(
            ".generation-{}.exact.pending",
            generation.active_epoch
        ));
        write_durable_pending(&temporary_path, &segment, "workspace generation").await?;
        write_durable_pending(
            &exact_temporary_path,
            &exact_segment,
            "workspace exact projection",
        )
        .await?;
        fs::rename(&temporary_path, &final_path)
            .await
            .map_err(|error| format!("publish workspace generation segment: {error}"))?;
        fs::rename(&exact_temporary_path, &exact_path)
            .await
            .map_err(|error| format!("publish workspace exact projection segment: {error}"))?;
        fs::File::open(&self.directory)
            .await
            .map_err(|error| format!("open workspace generation directory: {error}"))?
            .sync_all()
            .await
            .map_err(|error| format!("sync workspace generation directory: {error}"))?;
        let qualified_digest = |digest: &str| {
            if digest.starts_with("blake3-256:") {
                digest.to_owned()
            } else {
                format!("blake3-256:{digest}")
            }
        };
        let snapshot = WorkspaceGenerationSnapshot {
            schema_id: WORKSPACE_GENERATION_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            state: WorkspaceGenerationState::Ready,
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
            root_depth: generation.root_depth,
            source_kind: generation.source_snapshot.source_kind,
            leaf_count: generation.workspace_generation.leaf_count,
            owner_count: generation.workspace_generation.owner_count,
            provider_schema_digest: generation.provider_schema_digest.clone(),
            source_root_digest: qualified_digest(&generation.source_snapshot.root_digest),
            base_root_digest: generation
                .source_snapshot
                .base_root_digest
                .as_deref()
                .map(qualified_digest),
            source_provider_digest: qualified_digest(&generation.source_snapshot.provider_digest),
            dirty_paths_digest: generation
                .source_snapshot
                .dirty_paths_digest
                .as_deref()
                .map(qualified_digest),
            module_graph_digest: generation.module_graph_digest.clone(),
            selector_set_digest: generation.selector_set_digest.clone(),
            memory_backend_digest: generation.memory_backend_digest.clone(),
            workspace_source_scope_generation: generation.workspace_source_scope_generation.clone(),
            durable_commit_digest,
            mmap_segment_path: final_path.to_string_lossy().into_owned(),
            previous_epoch_readable,
        };
        snapshot.validate()?;
        super::publish_search_generation_authority_segment(state.pointer.path(), &generation)
            .await?;
        state.pointer.publish(&snapshot).await?;
        super::WorkspaceGenerationDataPlaneClient::invalidate_committed_pointer(
            state.pointer.path(),
        );
        super::WorkspaceExactProjectionDataPlaneClient::prime_committed_pointer(
            state.pointer.path(),
        )
        .await?;
        state
            .owner_identity_journal
            .rebase(&snapshot.workspace_identity, &snapshot.generation_digest)
            .await?;
        *active_snapshot = Some(snapshot.clone());
        record_generation_stage(
            "generation-durable-publish",
            &generation,
            durable_publish_started.elapsed(),
            u64::try_from(segment.len()).ok(),
        );
        Ok(snapshot)
    }

    pub fn pointer_path(&self) -> &Path {
        &self.pointer_path
    }
}

fn record_generation_stage(
    stage: &str,
    generation: &WorkspaceMemoryGeneration,
    elapsed: std::time::Duration,
    canonical_encoded_bytes: Option<u64>,
) {
    let elapsed_micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
    let budget_micros = 800_000;
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-generation-publication",
        stage,
        elapsed_micros,
        budget_micros,
        if elapsed_micros < budget_micros {
            "within-budget"
        } else {
            "budget-exceeded"
        },
    );
    observation.workspace_identity = Some(generation.workspace_identity.clone());
    observation.generation_digest = Some(generation.generation_digest.clone());
    observation.canonical_encoded_bytes = canonical_encoded_bytes;
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
}

async fn prune_obsolete_generation_segments(
    directory: &Path,
    readable_generation_path: Option<&Path>,
) -> Result<(), String> {
    let readable_exact_path =
        readable_generation_path.map(super::exact_segment::exact_projection_segment_path);
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|error| format!("inspect workspace generation retention: {error}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| format!("read workspace generation retention entry: {error}"))?
    {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with("generation-") || !file_name.ends_with(".mmap") {
            continue;
        }
        if readable_generation_path.is_some_and(|readable| path == readable)
            || readable_exact_path
                .as_ref()
                .is_some_and(|readable| path == *readable)
        {
            continue;
        }
        fs::remove_file(&path).await.map_err(|error| {
            format!(
                "remove superseded workspace generation segment `{}`: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

async fn write_durable_pending(path: &Path, bytes: &[u8], context: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .await
        .map_err(|error| format!("open {context} pending segment: {error}"))?;
    file.write_all(bytes)
        .await
        .map_err(|error| format!("write {context} pending segment: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("sync {context} pending segment: {error}"))
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
    if mapping.len() < SEGMENT_HEADER_LEN
        || !agent_semantic_content_identity::workspace_memory_generation_segment::has_current_workspace_memory_generation_contract(mapping)
    {
        return Err("workspace generation segment contract is stale or invalid".to_owned());
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
