//! Public refresh API for the DB Engine source index.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProjectContext, ProviderId, ProviderRegistryEvidence,
    ProviderRegistrySnapshot, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::ClientDbEngineWriteSession;
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexRefreshRequest,
    client_db_source_index_file_count, source_index_file_hashes,
    source_index_import_with_file_hashes,
};
use agent_semantic_runtime::{collect_runtime_source_index_files, runtime_source_index_context};

use super::collect::collect_source_index_files;
use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_FILE_LIMIT, SOURCE_INDEX_PROVIDER_ID,
    SOURCE_INDEX_SCHEMA_ID, SOURCE_INDEX_SCHEMA_VERSION,
};
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

/// Refresh the DB Engine source index from the complete provider-owned source scope.
pub fn refresh_source_index(
    project_root: &Path,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let trace_started = Instant::now();
    let cache_report =
        agent_semantic_client_core::ClientCacheManifest::inspect_project(project_root);
    source_index_trace("cache-inspected", trace_started);
    let Some(cache_root) = cache_report.cache_root.as_ref() else {
        source_index_trace("cache-root-absent-warm-check", trace_started);
        return Ok(None);
    };
    if ClientDbEngine::inspect_client_dir(cache_root).status
        != agent_semantic_client_core::ClientDbStatus::Present
    {
        source_index_trace("db-absent-warm-check", trace_started);
        return Ok(None);
    }
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    source_index_trace("previous-file-hashes-loaded", trace_started);
    if previous_file_hashes.is_none() {
        source_index_trace("generation-absent-warm-check", trace_started);
        return Ok(None);
    }
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let files = collect_source_index_files(project_root, &snapshot)?;
    source_index_trace("scope-files-collected", trace_started);
    let report = context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })?;
    source_index_trace("generation-refreshed", trace_started);
    Ok(Some(report))
}

fn source_index_snapshot_from_files(
    index_root: &Path,
    files: &[SourceIndexScopeFile],
    previous_file_hashes: Option<&[ClientCacheFileHash]>,
    registry: &ProviderRegistryEvidence,
) -> Result<
    (
        Vec<ClientCacheFileHash>,
        agent_semantic_artifacts::WorkspaceSnapshot,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        std::collections::BTreeMap<String, Vec<u8>>,
    ),
    String,
> {
    let file_hashes = source_index_file_hashes(
        index_root,
        files,
        previous_file_hashes,
        &registry.fingerprint,
        registry.scope_dirs.iter().map(String::as_str),
    )?;
    let mut workspace_file_hashes = Vec::with_capacity(files.len());
    let mut source_blobs = std::collections::BTreeMap::new();
    for file in files {
        let source_path = if file.path.is_absolute() {
            file.path.clone()
        } else {
            index_root.join(&file.path)
        };
        let bytes = std::fs::read(&source_path).map_err(|error| {
            format!(
                "failed to hash workspace source {} with BLAKE3: {error}",
                source_path.display()
            )
        })?;
        let snapshot_path = source_path
            .strip_prefix(index_root)
            .unwrap_or(source_path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        workspace_file_hashes.push((
            snapshot_path.clone(),
            blake3::hash(&bytes).to_hex().to_string(),
        ));
        source_blobs.insert(snapshot_path, bytes);
    }
    let workspace_snapshot =
        agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(workspace_file_hashes);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        agent_semantic_artifacts::provider_digest(registry.fingerprint.as_bytes()),
    );
    Ok((
        file_hashes,
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    ))
}

/// One content-authoritative view of the live workspace for all source
/// acquisition paths in a request.
pub struct CurrentSourceIndexSnapshot {
    pub workspace_snapshot: agent_semantic_artifacts::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Owner bytes captured in the same read pass that produced the Merkle root.
    pub source_blobs: std::collections::BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexOwnerPath(String);

impl SourceIndexOwnerPath {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SourceIndexOwnerPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SourceIndexOwnerPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSourceSnapshotEnvelopeV1<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    provider_id: &'a str,
    source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    cas_root: &'a Path,
    owners: Vec<ProviderSourceSnapshotOwnerV1>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSourceSnapshotOwnerV1 {
    path: String,
    snapshot_leaf_digest: String,
    blob_digest: String,
    source_content_digest: String,
    cas_path: String,
}

/// Publish one provider-scoped, root-bound source envelope backed by ASP-owned CAS bytes.
pub fn publish_provider_source_snapshot_envelope(
    snapshot: &CurrentSourceIndexSnapshot,
    provider_id: impl Into<ProviderId>,
    source_extensions: &[String],
    cache_home: &Path,
) -> Result<std::path::PathBuf, String> {
    let provider_id = provider_id.into();
    let cas_root = cache_home.join("source-blob-cas").join("v1");
    let cas = agent_semantic_content_identity::ContentAddressedStore::new(&cas_root);
    let normalized_extensions = source_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    let mut owners = Vec::new();
    for (path, bytes) in &snapshot.source_blobs {
        let extension = Path::new(path)
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
        if !normalized_extensions.is_empty()
            && extension
                .as_ref()
                .is_none_or(|extension| !normalized_extensions.contains(extension))
        {
            continue;
        }
        let snapshot_leaf_digest = snapshot.workspace_snapshot.file_digest(path).ok_or_else(|| {
            format!(
                "provider source blob is not committed by snapshot root: path={path} rootDigest={}",
                snapshot.source_snapshot.root_digest
            )
        })?;
        let blob_digest = agent_semantic_content_identity::hash_blob(bytes).value;
        let source_content_digest =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(bytes)
                .as_str()
                .to_owned();
        let blob_path = cas.write(&blob_digest, bytes).map_err(|error| {
            format!(
                "failed to publish provider source blob to ASP content store: path={path} blobDigest={blob_digest} error={error}"
            )
        })?;
        let cas_path = blob_path
            .strip_prefix(&cas_root)
            .map_err(|error| {
                format!(
                    "provider source blob escaped ASP content store: path={} casRoot={} error={error}",
                    blob_path.display(),
                    cas_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        owners.push(ProviderSourceSnapshotOwnerV1 {
            path: path.clone(),
            snapshot_leaf_digest: snapshot_leaf_digest.to_string(),
            blob_digest,
            source_content_digest,
            cas_path,
        });
    }
    owners.sort_by(|left, right| left.path.cmp(&right.path));
    let envelope_dir = cache_home
        .join("source-snapshot-envelopes")
        .join("v1")
        .join(&snapshot.source_snapshot.root_digest);
    std::fs::create_dir_all(&envelope_dir).map_err(|error| {
        format!(
            "failed to create provider source envelope directory {}: {error}",
            envelope_dir.display()
        )
    })?;
    let provider_file_name = provider_id
        .as_str()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let envelope_path = envelope_dir.join(format!("{provider_file_name}.json"));
    let envelope = ProviderSourceSnapshotEnvelopeV1 {
        schema_id: "asp.exact-source-snapshot-envelope.v1",
        schema_version: "1",
        provider_id: provider_id.as_str(),
        source_snapshot: &snapshot.source_snapshot,
        cas_root: &cas_root,
        owners,
    };
    let bytes = serde_json::to_vec_pretty(&envelope)
        .map_err(|error| format!("failed to encode provider source snapshot envelope: {error}"))?;
    let temporary = envelope_dir.join(format!(".{provider_file_name}.tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "failed to write provider source snapshot envelope {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &envelope_path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!(
            "failed to publish provider source snapshot envelope {}: {error}",
            envelope_path.display()
        )
    })?;
    Ok(envelope_path)
}

/// Capture the current content-authoritative source snapshot used by both
/// source-index rebuild and lookup.
pub fn current_source_index_snapshot(
    project_root: &Path,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::load(project_root)?;
    current_source_index_snapshot_with_registry(project_root, &provider_registry)
}

/// Capture a content-authoritative, one-owner snapshot for an exact query.
///
/// Exact reads are an owner-scoped evidence projection. They must not rebuild
/// the workspace source index or scan unrelated owners before invoking the
/// live parser.
pub fn current_source_index_snapshot_for_owner(
    project_root: &Path,
    owner_path: impl Into<SourceIndexOwnerPath>,
    language_id: impl Into<LanguageId>,
    provider_id: impl Into<ProviderId>,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let owner_path = owner_path.into();
    let language_id = language_id.into();
    let provider_id = provider_id.into();
    let provider_registry = ProviderRegistrySnapshot::load(project_root)?;
    current_source_index_snapshot_for_owner_with_registry(
        project_root,
        owner_path.as_str(),
        language_id.as_str(),
        provider_id.as_str(),
        &provider_registry,
    )
}

/// Capture one exact owner from an activation that the caller already loaded.
///
/// This keeps activation synchronization at the command boundary instead of
/// re-entering the manifest/activation materializer from the source snapshot.
pub fn current_source_index_snapshot_for_owner_from_activation(
    project_root: &Path,
    activation_path: &Path,
    activation: &agent_semantic_hook::HookRuntime,
    owner_path: impl Into<SourceIndexOwnerPath>,
    language_id: impl Into<LanguageId>,
    provider_id: impl Into<ProviderId>,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let owner_path = owner_path.into();
    let language_id = language_id.into();
    let provider_id = provider_id.into();
    let provider_registry = ProviderRegistrySnapshot::from_activation(activation_path, activation)?;
    current_source_index_snapshot_for_owner_with_registry(
        project_root,
        owner_path.as_str(),
        language_id.as_str(),
        provider_id.as_str(),
        &provider_registry,
    )
}

fn current_source_index_snapshot_for_owner_with_registry(
    project_root: &Path,
    owner_path: &str,
    language_id: &str,
    provider_id: &str,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let registry = provider_registry.evidence(project_root);
    let owner_path = explicit_snapshot_owner_path(project_root, owner_path)?;
    let files = [SourceIndexScopeFile {
        path: owner_path,
        language_id: LanguageId::from(language_id),
        provider_id: ProviderId::from(provider_id),
        selector_receipts: Vec::new(),
    }];
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, None, &registry)?;
    Ok(CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    })
}

fn explicit_snapshot_owner_path(project_root: &Path, owner_path: &str) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(owner_path).components() {
        match component {
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!(
                    "exact source owner escaped workspace namespace: ownerPath={owner_path} reasonKind=owner-outside-workspace"
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(
            "exact source owner path is empty: reasonKind=owner-not-in-worktree".to_string(),
        );
    }
    let canonical_root = project_root.canonicalize().map_err(|error| {
        format!(
            "failed to resolve exact source workspace {}: {error}",
            project_root.display()
        )
    })?;
    let source_path = project_root.join(&normalized);
    let canonical_source = source_path.canonicalize().map_err(|error| {
        format!(
            "exact source owner is not available in workspace: ownerPath={} reasonKind=owner-not-in-worktree error={error}",
            normalized.display()
        )
    })?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(format!(
            "exact source owner escaped workspace namespace: ownerPath={} reasonKind=owner-outside-workspace",
            normalized.display()
        ));
    }
    if !source_path.is_file() {
        return Err(format!(
            "exact source owner is not a file: ownerPath={} reasonKind=owner-not-in-worktree",
            normalized.display()
        ));
    }
    Ok(normalized)
}

pub(crate) fn current_source_index_snapshot_with_registry(
    project_root: &Path,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let registry = provider_registry.evidence(project_root);
    let files = collect_source_index_files(project_root, &provider_registry)?;
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, None, &registry)?;
    Ok(CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    })
}

/// Capture the current content-authoritative snapshot for an ASP-managed
/// runtime source checkout using the same identity inputs as its source index.
pub(crate) fn current_runtime_source_index_snapshot(
    project_root: &Path,
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let db_engine = ClientDbEngine::resolve(project_root)?;
    let runtime_context = runtime_source_index_context(
        (
            checkout_root,
            db_engine.client_dir(),
            language_id.as_str(),
            provider_id.as_str(),
        )
            .into(),
    )?;
    let files = collect_runtime_source_index_files(
        (
            runtime_context.checkout_root.as_path(),
            language_id.as_str(),
            provider_id.as_str(),
            SOURCE_INDEX_FILE_LIMIT,
        )
            .into(),
    )?
    .into_iter()
    .map(|file| SourceIndexScopeFile {
        path: file.path,
        language_id: LanguageId::from(file.language_id),
        provider_id: ProviderId::from(file.provider_id),
        selector_receipts: Vec::new(),
    })
    .collect::<Vec<_>>();
    if files.is_empty() {
        return Err(format!(
            "runtime source snapshot found no source files in {} for language {}",
            runtime_context.checkout_root.display(),
            language_id
        ));
    }
    let registry = ProviderRegistryEvidence {
        fingerprint: runtime_context.registry_fingerprint,
        scope_dirs: BTreeSet::new(),
    };
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(&runtime_context.checkout_root, &files, None, &registry)?;
    Ok(CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    })
}

/// Rebuild the DB Engine source index for a project without storing raw source.
pub fn rebuild_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let trace_started = Instant::now();
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    source_index_trace("previous-file-hashes-loaded", trace_started);
    let files = collect_source_index_files(project_root, &snapshot)?;
    source_index_trace("scope-files-collected", trace_started);
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })
}

/// Refresh source-index rows for an ASP-managed runtime source checkout.
pub fn refresh_runtime_source_index(
    project_root: &Path,
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<SourceIndexRefreshReport, String> {
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    let client_cache_dir = context.client_cache_dir();
    let runtime_context = runtime_source_index_context(
        (
            checkout_root,
            client_cache_dir,
            language_id.as_str(),
            provider_id.as_str(),
        )
            .into(),
    )?;

    let previous_file_hashes = context.latest_file_hashes(&runtime_context.checkout_root)?;
    let files = collect_runtime_source_index_files(
        (
            runtime_context.checkout_root.as_path(),
            language_id.as_str(),
            provider_id.as_str(),
            SOURCE_INDEX_FILE_LIMIT,
        )
            .into(),
    )?
    .into_iter()
    .map(|file| SourceIndexScopeFile {
        path: file.path,
        language_id: LanguageId::from(file.language_id),
        provider_id: ProviderId::from(file.provider_id),
        selector_receipts: Vec::new(),
    })
    .collect::<Vec<_>>();
    if files.is_empty() {
        return Err(format!(
            "runtime source index found no source files in {} for language {}",
            runtime_context.checkout_root.display(),
            language_id
        ));
    }
    let registry = ProviderRegistryEvidence {
        fingerprint: runtime_context.registry_fingerprint,
        scope_dirs: BTreeSet::new(),
    };
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: &runtime_context.checkout_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })
}

struct SourceIndexRefreshContext {
    db_path: std::path::PathBuf,
    client_cache_dir: std::path::PathBuf,
    db_session: ClientDbEngineWriteSession,
    schema_id: SemanticSchemaId,
    schema_version: SemanticSchemaVersion,
}

impl SourceIndexRefreshContext {
    fn resolve(project_root: &Path) -> Result<Self, String> {
        let project_context = ProjectContext::resolve(project_root)?;
        project_context.require_inside_workspace(project_root)?;
        let db_engine = ClientDbEngine::resolve(project_root)?;
        let db_path = db_engine.db_path().to_path_buf();
        let client_cache_dir = db_engine.client_dir().to_path_buf();
        let db_session = ClientDbEngine::open_write_session_client_dir(db_engine.client_dir())?;
        Ok(Self {
            db_path,
            client_cache_dir,
            db_session,
            schema_id: SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION),
        })
    }

    fn client_cache_dir(&self) -> &Path {
        &self.client_cache_dir
    }

    fn latest_file_hashes(
        &self,
        index_root: &Path,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        self.db_session.latest_source_index_file_hashes(
            index_root,
            &self.schema_id,
            &self.schema_version,
        )
    }

    fn refresh_generation(
        &mut self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<SourceIndexRefreshReport, String> {
        let trace_started = Instant::now();
        let (file_hashes, workspace_snapshot, mut source_snapshot, _) =
            source_index_snapshot_from_files(
                request.index_root,
                request.files,
                request.previous_file_hashes,
                request.registry,
            )?;
        source_index_trace("generation-file-hashes-built", trace_started);
        let reusable_stats = self.db_session.reusable_source_index_generation(
            request.index_root,
            &self.schema_id,
            &self.schema_version,
            &file_hashes,
        )?;
        source_index_trace("generation-reuse-checked", trace_started);
        if let Some(stats) = reusable_stats {
            return Ok(source_index_refresh_report(
                &self.db_path,
                stats,
                request.files.len(),
                true,
            ));
        }
        let previous_stats = self.db_session.latest_source_index_stats(
            request.index_root,
            &self.schema_id,
            &self.schema_version,
        )?;
        let previous_scope_files = self.db_session.latest_source_index_scope_files(
            request.index_root,
            &self.schema_id,
            &self.schema_version,
        )?;
        let generation_id =
            agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
                &source_snapshot,
            );
        let import = source_index_import_with_file_hashes(
            ClientDbSourceIndexImportAssemblyRequest {
                generation_id,
                project_root: request.index_root.to_path_buf(),
                schema_id: self.schema_id.clone(),
                schema_version: self.schema_version.clone(),
                selector_source: SOURCE_INDEX_PROVIDER_ID.into(),
                file_text_bytes_limit: SOURCE_INDEX_FILE_BYTES_LIMIT,
                previous_file_hashes: None,
                registry_fingerprint: request.registry.fingerprint.clone(),
                extra_scope_dirs: request.registry.scope_dirs.iter().cloned().collect(),
                files: request.files.to_vec(),
            },
            file_hashes,
        )?;
        let membership_change_set = match (
            previous_stats,
            request.previous_file_hashes,
            previous_scope_files,
        ) {
            (Some(previous_stats), Some(previous_file_hashes), Some(previous_scope_files)) => {
                let previous_hashes = previous_file_hashes
                    .iter()
                    .map(|file| (file.path.as_str(), file.sha256.as_str()))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let current_hashes = import
                    .file_hashes
                    .iter()
                    .map(|file| (file.path.as_str(), file.sha256.as_str()))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let current_owner_paths = import
                    .owners
                    .iter()
                    .map(|owner| owner.owner_path.as_str().to_string())
                    .collect::<std::collections::BTreeSet<_>>();
                let changed_owner_paths = current_owner_paths
                    .iter()
                    .filter(|path| {
                        previous_hashes.get(path.as_str()) != current_hashes.get(path.as_str())
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let removed_owner_paths = previous_scope_files
                    .iter()
                    .map(|file| {
                        agent_semantic_client_db::source_index_relative_path(
                            request.index_root,
                            &file.path,
                        )
                    })
                    .filter(|path| !current_owner_paths.contains(path))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if changed_owner_paths.is_empty() && removed_owner_paths.is_empty() {
                    agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::FullSnapshot
                } else {
                    source_snapshot = workspace_snapshot.overlay_evidence(
                        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
                        source_snapshot.provider_digest.clone(),
                        previous_stats.source_snapshot.root_digest,
                        changed_owner_paths.iter().cloned(),
                        removed_owner_paths.iter().cloned(),
                    )?;
                    agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                        changed_owner_paths: changed_owner_paths
                            .into_iter()
                            .map(agent_semantic_client_db::ClientDbSourceIndexPath::new)
                            .collect(),
                        removed_owner_paths: removed_owner_paths
                            .into_iter()
                            .map(agent_semantic_client_db::ClientDbSourceIndexPath::new)
                            .collect(),
                    }
                }
            }
            _ => agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        };
        source_index_trace("generation-import-assembled", trace_started);
        let report =
            self.db_session
                .refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
                    import,
                    file_count: client_db_source_index_file_count(request.files.len()),
                    source_snapshot: source_snapshot.clone(),
                    membership_change_set,
                })?;
        source_index_trace("generation-turso-imported", trace_started);
        Ok(SourceIndexRefreshReport::from_report(
            self.db_path.clone(),
            report,
            source_snapshot,
        ))
    }
}

fn source_index_trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

struct SourceIndexGenerationRefresh<'a> {
    index_root: &'a Path,
    files: &'a [SourceIndexScopeFile],
    previous_file_hashes: Option<&'a [ClientCacheFileHash]>,
    registry: &'a ProviderRegistryEvidence,
}

fn source_index_refresh_report(
    db_path: &Path,
    stats: agent_semantic_client_db::ClientDbSourceIndexStats,
    file_count: usize,
    reused_generation: bool,
) -> SourceIndexRefreshReport {
    let source_snapshot = stats.source_snapshot.clone();
    SourceIndexRefreshReport::from_report(
        db_path.to_path_buf(),
        agent_semantic_client_db::ClientDbSourceIndexRefreshReport {
            generation_id: stats.generation_id,
            reused_generation,
            file_count: client_db_source_index_file_count(file_count),
            owner_count: stats.owner_count,
            selector_count: stats.selector_count,
            changed_owner_count: 0,
            removed_owner_count: 0,
            posting_write_count: 0,
        },
        source_snapshot,
    )
}
