//! Public refresh API for the DB Engine source index.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProjectContext, ProviderId, ProviderRegistryEvidence,
    ProviderRegistrySnapshot, SemanticSchemaId, SemanticSchemaVersion,
};
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
use super::provider_envelope::{
    ProviderSourceEnvelopeLookupRequestV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry,
};

/// Refresh the DB Engine source index from the complete provider-owned source scope.
pub fn refresh_source_index(
    project_root: &Path,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let trace_started = Instant::now();
    let Some(mut context) = source_index_refresh_context(project_root, trace_started)? else {
        return Ok(None);
    };
    let report =
        refresh_complete_source_index_generation(project_root, &mut context, trace_started)?;
    Ok(Some(report))
}

fn source_index_refresh_context(
    project_root: &Path,
    trace_started: Instant,
) -> Result<Option<SourceIndexRefreshContext>, String> {
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
    let context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    Ok(Some(context))
}

fn refresh_complete_source_index_generation(
    project_root: &Path,
    context: &mut SourceIndexRefreshContext,
    trace_started: Instant,
) -> Result<SourceIndexRefreshReport, String> {
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let files = collect_source_index_files(
        project_root,
        &snapshot,
        &super::collect::SourceIndexCollectionScope::CompleteGeneration,
    )?;
    source_index_trace("scope-files-collected", trace_started);
    let report = context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
        registry: &registry,
    })?;
    source_index_trace("generation-refreshed", trace_started);
    Ok(report)
}

fn source_index_snapshot_from_files(
    index_root: &Path,
    files: &[SourceIndexScopeFile],
    registry: &ProviderRegistryEvidence,
) -> Result<
    (
        Vec<ClientCacheFileHash>,
        agent_semantic_artifacts::WorkspaceSnapshot,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
    ),
    String,
> {
    let mut workspace_file_hashes = Vec::with_capacity(files.len());
    let mut source_blobs = Vec::with_capacity(files.len());
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
        source_blobs.push((
            agent_semantic_client_db::ClientDbSourceIndexPath::new(snapshot_path),
            bytes,
        ));
    }
    let typed_source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(source_blobs);
    let file_hashes = source_index_file_hashes(
        index_root,
        files,
        &typed_source_blobs,
        &registry.fingerprint,
        registry.scope_dirs.iter().map(String::as_str),
    )?;
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
        typed_source_blobs,
    ))
}

/// One content-authoritative view of the live workspace for all source
/// acquisition paths in a request.
pub struct CurrentSourceIndexSnapshot {
    pub workspace_snapshot: agent_semantic_artifacts::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Complete generation identity validated once when the snapshot is
    /// published or loaded.
    pub workspace_generation:
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
    /// Owner bytes captured in the same read pass that produced the Merkle root.
    pub source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
}

/// Canonical workspace-relative owner path used by source-index acquisition.
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

/// Capture the current content-authoritative source snapshot used by both
/// source-index rebuild and lookup.
pub fn current_source_index_snapshot(
    project_root: &Path,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::load(project_root)?;
    current_source_index_snapshot_with_registry(project_root, &provider_registry)
}

/// Capture a workspace-search snapshot from complete provider-owned coverage.
///
/// Missing provider owners fail closed; activation scope is never merged into
/// the provider snapshot.
pub fn current_workspace_search_source_index_snapshot(
    project_root: &Path,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::load(project_root)?;
    let registry = provider_registry.evidence(project_root);
    let files = super::collect::collect_workspace_search_source_index_files(
        project_root,
        &provider_registry,
        &super::collect::SourceIndexCollectionScope::CompleteGeneration,
    )?;
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
}

pub fn current_provider_source_index_snapshot_with_registry(
    project_root: &Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let project_context = ProjectContext::resolve(project_root)?;
    current_provider_source_index_snapshot_at_artifact_root_with_registry(
        ProviderSourceEnvelopeLookupRequestV1 {
            project_root,
            artifact_root: project_context.state_layout().artifacts_dir(),
            language_id,
            provider_id,
            provider_registry,
        },
    )
}

/// Capture the current worktree snapshot for exactly one registered provider.
///
/// This is the rootDepth=0 query boundary. It performs no envelope
/// publication, CAS write, database bootstrap, or activation synchronization.
pub fn current_live_provider_source_index_snapshot_with_registry(
    project_root: &Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let registry = provider_registry.evidence(project_root);
    let files = collect_source_index_files(
        project_root,
        provider_registry,
        &super::collect::SourceIndexCollectionScope::TargetProvider {
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
        },
    )?;
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedProviderSourceEnvelope {
    schema_id: String,
    schema_version: String,
    provider_id: String,
    provider_workspace_root: String,
    provider_workspace_identity_digest: String,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    materialization_state: String,
    owner_coverage: String,
    cas_root: PathBuf,
    owners: Vec<PublishedProviderSourceOwner>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedProviderSourceOwner {
    path: String,
    snapshot_leaf_digest: String,
    blob_digest: String,
    source_content_digest: String,
    cas_path: String,
}

pub(super) fn load_provider_source_index_snapshot_envelope(
    artifact_root: &Path,
    requested_provider_id: &str,
    expected_provider_digest: &str,
    expected_provider_workspace_identity: &super::provider_envelope::ProviderWorkspaceIdentityV1,
    envelope_path: &Path,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let bytes = std::fs::read(envelope_path).map_err(|error| {
        format!(
            "failed to read requested provider source envelope {}: {error}",
            envelope_path.display()
        )
    })?;
    let envelope: PublishedProviderSourceEnvelope =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "invalid requested provider source envelope {}: {error}",
                envelope_path.display()
            )
        })?;
    let invalid_reason = if envelope.schema_id != "asp.exact-source-snapshot-envelope.v1" {
        Some("schema-id")
    } else if envelope.schema_version != "1" {
        Some("schema-version")
    } else if envelope.provider_id != requested_provider_id {
        Some("provider-id")
    } else if envelope.provider_workspace_root != expected_provider_workspace_identity.root {
        Some("provider-workspace-root")
    } else if envelope.provider_workspace_identity_digest
        != expected_provider_workspace_identity.digest
    {
        Some("provider-workspace-identity-digest")
    } else if envelope.source_snapshot.provider_digest != expected_provider_digest {
        Some("provider-digest")
    } else if envelope.materialization_state != "artifact-complete" {
        Some("materialization-state")
    } else if envelope.owner_coverage != "complete" {
        Some("owner-coverage")
    } else if envelope.owners.is_empty() {
        Some("owners-empty")
    } else {
        None
    };
    if let Some(reason) = invalid_reason {
        return Err(provider_envelope_contract_error(
            requested_provider_id,
            reason,
        ));
    }
    let cas_root = artifact_root.join("source-blob-cas").join("v1");
    if envelope.cas_root != cas_root {
        return Err(provider_envelope_contract_error(
            requested_provider_id,
            "cas-root",
        ));
    }
    let mut workspace_hashes = Vec::with_capacity(envelope.owners.len());
    let mut source_blobs = Vec::with_capacity(envelope.owners.len());
    for owner in &envelope.owners {
        let relative_cas_path = normalized_envelope_relative_path(&owner.cas_path)?;
        let blob_path = cas_root.join(&relative_cas_path);
        let owner_bytes = std::fs::read(&blob_path).map_err(|error| {
            format!(
                "requested provider source envelope blob is missing: providerId={requested_provider_id} path={} error={error}",
                owner.path
            )
        })?;
        if agent_semantic_content_identity::hash_blob(&owner_bytes).value != owner.blob_digest
            || agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &owner_bytes,
            )
            .as_str()
                != owner.source_content_digest
        {
            return Err(format!(
                "requested provider source envelope blob digest is invalid: providerId={requested_provider_id} path={}",
                owner.path
            ));
        }
        workspace_hashes.push((owner.path.as_str(), owner.snapshot_leaf_digest.as_str()));
        source_blobs.push((
            agent_semantic_client_db::ClientDbSourceIndexPath::new(owner.path.clone()),
            owner_bytes,
        ));
    }
    let workspace_snapshot =
        agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(workspace_hashes);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        expected_provider_digest.to_owned(),
    );
    if source_snapshot.root_digest != envelope.source_snapshot.root_digest
        || source_snapshot.leaf_count != envelope.source_snapshot.leaf_count
    {
        return Err(format!(
            "requested provider source envelope snapshot is invalid: providerId={requested_provider_id}"
        ));
    }
    Ok(CurrentSourceIndexSnapshot {
        workspace_generation: materialized_workspace_generation(
            &source_snapshot,
            source_blobs.len(),
        )?,
        workspace_snapshot,
        source_snapshot,
        source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
            source_blobs,
        ),
    })
}

fn provider_envelope_contract_error(provider_id: &str, reason: &str) -> String {
    format!(
        "requested provider source envelope contract is invalid: providerId={provider_id} reason={reason}"
    )
}

fn normalized_envelope_relative_path(path: &str) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!(
                    "provider source envelope CAS path escaped artifact root: path={path}"
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("provider source envelope CAS path is empty".to_owned());
    }
    Ok(normalized)
}

pub(super) fn fresh_target_provider_source_index_snapshot_with_registry(
    project_root: &Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    collection_scope: &super::collect::SourceIndexCollectionScope,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let registry = provider_registry.evidence(project_root);
    let files = collect_source_index_files(project_root, provider_registry, collection_scope)?;
    if files.is_empty()
        || files
            .iter()
            .any(|file| &file.language_id != language_id || &file.provider_id != provider_id)
    {
        return Err(format!(
            "target provider source-scope output is incomplete: languageId={} providerId={}",
            language_id, provider_id
        ));
    }
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
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
        source_index_snapshot_from_files(project_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
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
    let files = collect_source_index_files(
        project_root,
        provider_registry,
        &super::collect::SourceIndexCollectionScope::CompleteGeneration,
    )?;
    let (_, workspace_snapshot, source_snapshot, source_blobs) =
        source_index_snapshot_from_files(project_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
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
        source_index_snapshot_from_files(&runtime_context.checkout_root, &files, &registry)?;
    materialized_current_source_index_snapshot(workspace_snapshot, source_snapshot, source_blobs)
}

/// Rebuild the DB Engine source index for a project without storing raw source.
pub fn rebuild_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let trace_started = Instant::now();
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    source_index_trace("rebuild-mode-selected", trace_started);
    let files = collect_source_index_files(
        project_root,
        &snapshot,
        &super::collect::SourceIndexCollectionScope::CompleteGeneration,
    )?;
    source_index_trace("scope-files-collected", trace_started);
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
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
        registry: &registry,
    })
}

struct SourceIndexRefreshContext {
    db_path: std::path::PathBuf,
    client_cache_dir: std::path::PathBuf,
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
        Ok(Self {
            db_path,
            client_cache_dir,
            schema_id: SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION),
        })
    }

    fn client_cache_dir(&self) -> &Path {
        &self.client_cache_dir
    }

    fn refresh_generation(
        &mut self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<SourceIndexRefreshReport, String> {
        let trace_started = Instant::now();
        let (file_hashes, _workspace_snapshot, source_snapshot, source_blobs) =
            source_index_snapshot_from_files(request.index_root, request.files, request.registry)?;
        source_index_trace("generation-file-hashes-built", trace_started);
        source_index_trace("generation-evidence-built", trace_started);
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
                registry_fingerprint: request.registry.fingerprint.clone(),
                extra_scope_dirs: request.registry.scope_dirs.iter().cloned().collect(),
                files: request.files.to_vec(),
                source_blobs,
            },
            file_hashes,
        )?;
        source_index_trace("generation-import-assembled", trace_started);
        let refresh_request = ClientDbSourceIndexRefreshRequest {
            import,
            file_count: client_db_source_index_file_count(request.files.len()),
            source_snapshot: source_snapshot.clone(),
        };
        let report =
            agent_semantic_client_db::workspace_db_ipc::
                commit_source_index_generation_via_resident(refresh_request)?;
        source_index_trace("generation-turso-imported", trace_started);
        Ok(SourceIndexRefreshReport::from_report(
            self.db_path.clone(),
            report.clone(),
            report.source_snapshot,
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
    registry: &'a ProviderRegistryEvidence,
}

#[path = "activation_snapshot.rs"]
mod activation_snapshot;
#[cfg(test)]
#[path = "../../tests/unit/source_index_api.rs"]
mod tests;
pub use activation_snapshot::{
    CurrentSourceIndexOwnerFromActivationRequest,
    current_provider_source_index_snapshot_from_activation,
    current_source_index_snapshot_for_owner_from_activation,
    current_source_index_snapshot_from_activation,
    ensure_provider_source_index_snapshot_from_activation,
    provider_source_snapshot_envelope_path_from_activation,
};
pub(crate) fn materialized_current_source_index_snapshot(
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let workspace_generation =
        materialized_workspace_generation(&source_snapshot, source_blobs.len())?;
    Ok(CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot,
        workspace_generation,
        source_blobs,
    })
}

fn materialized_workspace_generation(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    owner_count: usize,
) -> Result<
    agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
    String,
> {
    agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
            root_digest: source_snapshot.root_digest.clone(),
            root_depth: 1,
            leaf_count: source_snapshot.leaf_count as u64,
            owner_count: owner_count as u64,
        },
    )
    .map_err(|error| error.to_string())
}
