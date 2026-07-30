//! ASP-owned fast path for `search owner <path> items`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::language_owner_items_workspace_root;
use agent_semantic_client_db::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderIncrementalWriteReceipt,
    ProviderOwnerBatchProbeRequest, ProviderOwnerDecision, ProviderOwnerFingerprint,
    ProviderOwnerMetadata, ProviderOwnerProbe, ProviderSelectorProjection,
};

pub(super) struct SearchOwnerItemsFastContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

struct OwnerItemsSearchState<'a> {
    language_id: &'a str,
    owner_project_root: PathBuf,
    incremental_scope: Option<ProviderIncrementalScoped>,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    owner: &'a Path,
    query: &'a str,
}

impl<'a> OwnerItemsSearchState<'a> {
    fn new(
        context: SearchOwnerItemsFastContext<'a>,
        owner: &'a Path,
        query: &'a str,
        owner_project_root: PathBuf,
    ) -> Result<Self, String> {
        let resolved_state_started = Instant::now();
        let incremental_scope = resolve_incremental_owner_state(&owner_project_root, &context)?;
        owner_items_trace("resolved-state", resolved_state_started);
        Ok(Self {
            language_id: context.language_id,
            owner_project_root,
            incremental_scope,
            provider_context: context.provider_context,
            owner,
            query,
        })
    }

    fn run_provider(&self) -> Result<(), String> {
        let handler_started = Instant::now();
        let (Some(scope), Some(provider_context)) =
            (self.incremental_scope.as_ref(), self.provider_context)
        else {
            return Err(format!(
                "parser-owned owner-items state is incomplete: reasonKind=incremental-owner-state-incomplete scope={} providerContext={}",
                self.incremental_scope.is_some(),
                self.provider_context.is_some(),
            ));
        };
        let runtime_started = Instant::now();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create provider incremental runtime: {error}"))?;
        owner_items_trace("runtime-build", runtime_started);
        let session_started = Instant::now();
        let session =
            super::runtime_server::runtime_server_workspace_session(&self.owner_project_root)?;
        owner_items_trace("workspace-session-acquire", session_started);
        let normalized_owner_started = Instant::now();
        let (owner_path, owner_key) = normalized_owner_path(&self.owner_project_root, self.owner)?;
        owner_items_trace("owner-normalized", normalized_owner_started);
        let metadata_started = Instant::now();
        let metadata = provider_owner_metadata(&owner_path)?;
        owner_items_trace("metadata", metadata_started);
        match lookup_owner_state(
            &runtime,
            &session,
            scope,
            owner_key.as_str(),
            &metadata,
            self.query,
        )? {
            OwnerItemsLookup::Warm(hit) => {
                render_warm_owner_items(owner_key.as_str(), self.query, &hit);
            }
            OwnerItemsLookup::Refresh(probe) => {
                let hit = refresh_owner_from_provider(RefreshOwnerRequest {
                    language_id: self.language_id,
                    owner_project_root: &self.owner_project_root,
                    runtime: &runtime,
                    session: &session,
                    scope,
                    provider_context,
                    owner_path: &owner_path,
                    owner_key: owner_key.as_str(),
                    metadata,
                    query: self.query,
                    probe,
                })?;
                render_refreshed_owner_items(owner_key.as_str(), self.query, &hit);
            }
        }
        owner_items_trace("handler-total", handler_started);
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerItemsExecutionReceipt {
    metadata_reads: u32,
    source_byte_reads: u32,
    provider_invocations: u32,
    provider_parses: u32,
    owner_index_writes: u32,
    cas_writes: u32,
    merkle_leaf_writes: u32,
    merkle_path_node_writes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerItemsWarmHit {
    generation: Option<String>,
    projections: Vec<ProviderSelectorProjection>,
    receipt: OwnerItemsExecutionReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnerItemsLookup {
    Warm(OwnerItemsWarmHit),
    Refresh(ProviderOwnerProbe),
}

fn lookup_owner_state(
    runtime: &tokio::runtime::Runtime,
    session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    scope: &ProviderIncrementalScoped,
    owner_path: &str,
    metadata: &ProviderOwnerMetadata,
    query: &str,
) -> Result<OwnerItemsLookup, String> {
    let owner_state_started = Instant::now();
    let probe_receipt = runtime.block_on(session.probe_provider_owners(
        scope,
        &[ProviderOwnerBatchProbeRequest {
            owner_path: owner_path.to_owned(),
            metadata: metadata.clone(),
        }],
    ))?;
    let probe = probe_receipt
        .results
        .into_iter()
        .next()
        .ok_or_else(|| "provider owner batch probe returned no result".to_owned())?
        .probe;
    owner_items_trace("turso-owner-state", owner_state_started);
    if probe.decision != ProviderOwnerDecision::Unchanged {
        return Ok(OwnerItemsLookup::Refresh(probe));
    }
    let filter_started = Instant::now();
    let projections = runtime
        .block_on(session.read_provider_owner_projections(scope, owner_path))?
        .into_iter()
        .filter(|projection| projection_matches_query(projection, query))
        .collect();
    owner_items_trace("filter", filter_started);
    Ok(OwnerItemsLookup::Warm(OwnerItemsWarmHit {
        generation: probe.generation_before,
        projections,
        receipt: zero_side_effect_warm_receipt(),
    }))
}

fn zero_side_effect_warm_receipt() -> OwnerItemsExecutionReceipt {
    OwnerItemsExecutionReceipt {
        metadata_reads: 1,
        source_byte_reads: 0,
        provider_invocations: 0,
        provider_parses: 0,
        owner_index_writes: 0,
        cas_writes: 0,
        merkle_leaf_writes: 0,
        merkle_path_node_writes: 0,
    }
}

fn projection_matches_query(projection: &ProviderSelectorProjection, query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    query.is_empty()
        || projection
            .item_name
            .to_ascii_lowercase()
            .contains(query.as_str())
        || projection
            .signature
            .to_ascii_lowercase()
            .contains(query.as_str())
}

fn render_warm_owner_items(owner_path: &str, query: &str, hit: &OwnerItemsWarmHit) {
    let render_started = Instant::now();
    for projection in &hit.projections {
        println!(
            "{} selector={}",
            projection.signature, projection.structural_selector
        );
    }
    owner_items_trace("render", render_started);
    eprintln!(
        "[provider-incremental-owner] state=warm owner={} query={:?} matches={} generation={} metadataReads={} sourceByteReads={} providerInvocations={} providerParses={} ownerIndexWrites={} casWrites={} merkleLeafWrites={} merklePathNodeWrites={} fullWorkspaceReads=0 fullMerkleRebuilds=0 unrelatedProviderCount=0",
        owner_path,
        query,
        hit.projections.len(),
        hit.generation.as_deref().unwrap_or("none"),
        hit.receipt.metadata_reads,
        hit.receipt.source_byte_reads,
        hit.receipt.provider_invocations,
        hit.receipt.provider_parses,
        hit.receipt.owner_index_writes,
        hit.receipt.cas_writes,
        hit.receipt.merkle_leaf_writes,
        hit.receipt.merkle_path_node_writes,
    );
}

struct RefreshOwnerRequest<'a> {
    language_id: &'a str,
    owner_project_root: &'a Path,
    runtime: &'a tokio::runtime::Runtime,
    session: &'a agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    scope: &'a ProviderIncrementalScoped,
    provider_context: &'a ProviderGraphFactsContext<'a>,
    owner_path: &'a Path,
    owner_key: &'a str,
    metadata: ProviderOwnerMetadata,
    query: &'a str,
    probe: ProviderOwnerProbe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerItemsRefreshHit {
    decision: ProviderOwnerDecision,
    generation_before: Option<String>,
    generation_after: String,
    projections: Vec<ProviderSelectorProjection>,
    receipt: OwnerItemsExecutionReceipt,
}

fn refresh_owner_from_provider(
    request: RefreshOwnerRequest<'_>,
) -> Result<OwnerItemsRefreshHit, String> {
    let source_bytes = fs::read(request.owner_path).map_err(|error| {
        format!(
            "failed to read requested owner {}: {error}",
            request.owner_path.display()
        )
    })?;
    let content_digest =
        agent_semantic_content_identity::ArtifactHash::blake3(source_bytes.as_slice()).value;
    let fingerprint = ProviderOwnerFingerprint {
        metadata: request.metadata.clone(),
        content_digest: content_digest.clone(),
    };
    let projections = super::provider_owner_native::run_provider_owner_native(
        super::provider_owner_native::ProviderOwnerNativeTransportContext {
            language_id: request.language_id,
            provider: request.provider_context.provider,
            profiles: request.provider_context.profiles,
            project_root: request.owner_project_root,
        },
        super::provider_owner_native::ProviderOwnerNativeRequest {
            scope: request.scope,
            owner_path: request.owner_key,
            fingerprint: &fingerprint,
            source_bytes: source_bytes.as_slice(),
        },
    )?;
    commit_complete_owner_response(CommitOwnerRequest {
        runtime: request.runtime,
        session: request.session,
        scope: request.scope,
        owner_path: request.owner_key,
        fingerprint,
        query: request.query,
        decision: request.probe.decision,
        projections,
    })
}

struct CommitOwnerRequest<'a> {
    runtime: &'a tokio::runtime::Runtime,
    session: &'a agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    scope: &'a ProviderIncrementalScoped,
    owner_path: &'a str,
    fingerprint: ProviderOwnerFingerprint,
    query: &'a str,
    decision: ProviderOwnerDecision,
    projections: Vec<ProviderSelectorProjection>,
}

fn commit_complete_owner_response(
    request: CommitOwnerRequest<'_>,
) -> Result<OwnerItemsRefreshHit, String> {
    let filtered_projections = request
        .projections
        .iter()
        .filter(|projection| projection_matches_query(projection, request.query))
        .cloned()
        .collect();
    let write_receipt =
        request
            .runtime
            .block_on(request.session.write_provider_incremental_owner(
                &ProviderIncrementalOwnerWrite {
                    scope: request.scope.clone(),
                    owner_path: request.owner_path.to_string(),
                    fingerprint: request.fingerprint,
                    projection_completeness: "complete-owner".to_string(),
                    projections: request.projections,
                },
            ))?;
    Ok(refresh_hit_from_write(
        request.decision,
        filtered_projections,
        write_receipt,
    ))
}

fn refresh_hit_from_write(
    decision: ProviderOwnerDecision,
    projections: Vec<ProviderSelectorProjection>,
    write: ProviderIncrementalWriteReceipt,
) -> OwnerItemsRefreshHit {
    OwnerItemsRefreshHit {
        decision,
        generation_before: write.generation_before,
        generation_after: write.generation_after,
        projections,
        receipt: OwnerItemsExecutionReceipt {
            metadata_reads: 1,
            source_byte_reads: 1,
            provider_invocations: 1,
            provider_parses: 1,
            owner_index_writes: write.owner_index_writes,
            cas_writes: 0,
            merkle_leaf_writes: write.merkle_leaf_writes,
            merkle_path_node_writes: write.merkle_path_node_writes,
        },
    }
}

fn render_refreshed_owner_items(owner_path: &str, query: &str, hit: &OwnerItemsRefreshHit) {
    let render_started = Instant::now();
    for projection in &hit.projections {
        println!(
            "{} selector={}",
            projection.signature, projection.structural_selector
        );
    }
    owner_items_trace("render", render_started);
    let state = match hit.decision {
        ProviderOwnerDecision::New => "new",
        ProviderOwnerDecision::Changed => "changed",
        ProviderOwnerDecision::Unchanged => "warm",
    };
    eprintln!(
        "[provider-incremental-owner] state={} owner={} query={:?} matches={} generationBefore={} generationAfter={} metadataReads={} sourceByteReads={} providerInvocations={} providerParses={} ownerIndexWrites={} casWrites={} merkleLeafWrites={} merklePathNodeWrites={} fullWorkspaceReads=0 fullMerkleRebuilds=0 unrelatedProviderCount=0",
        state,
        owner_path,
        query,
        hit.projections.len(),
        hit.generation_before.as_deref().unwrap_or("none"),
        hit.generation_after,
        hit.receipt.metadata_reads,
        hit.receipt.source_byte_reads,
        hit.receipt.provider_invocations,
        hit.receipt.provider_parses,
        hit.receipt.owner_index_writes,
        hit.receipt.cas_writes,
        hit.receipt.merkle_leaf_writes,
        hit.receipt.merkle_path_node_writes,
    );
}

fn owner_items_trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_OWNER_ITEMS_TRACE").is_some() {
        eprintln!(
            "[provider-incremental-owner-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}

fn normalized_owner_path(
    owner_project_root: &Path,
    owner: &Path,
) -> Result<(PathBuf, String), String> {
    let canonical_root = owner_project_root
        .canonicalize()
        .map_err(|error| format!("failed to resolve owner workspace root: {error}"))?;
    let candidate = if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        canonical_root.join(owner)
    };
    let canonical_owner = candidate
        .canonicalize()
        .map_err(|error| format!("failed to resolve owner {}: {error}", candidate.display()))?;
    let relative = canonical_owner.strip_prefix(&canonical_root).map_err(|_| {
        format!(
            "owner {} is outside provider workspace {}",
            canonical_owner.display(),
            canonical_root.display()
        )
    })?;
    let owner_key = relative.to_string_lossy().replace('\\', "/");
    if owner_key.is_empty() {
        return Err("owner path must identify a file within the provider workspace".to_string());
    }
    Ok((canonical_owner, owner_key))
}

#[cfg(unix)]
fn provider_owner_metadata(path: &Path) -> Result<ProviderOwnerMetadata, String> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect owner metadata {}: {error}",
            path.display()
        )
    })?;
    Ok(ProviderOwnerMetadata {
        file_identity: format!("unix:{}:{}", metadata.dev(), metadata.ino()),
        size_bytes: metadata.len(),
        modified_unix_nanos: metadata
            .mtime()
            .saturating_mul(1_000_000_000)
            .saturating_add(metadata.mtime_nsec()),
        change_time_unix_nanos: metadata
            .ctime()
            .saturating_mul(1_000_000_000)
            .saturating_add(metadata.ctime_nsec()),
    })
}

#[cfg(not(unix))]
fn provider_owner_metadata(path: &Path) -> Result<ProviderOwnerMetadata, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect owner metadata {}: {error}",
            path.display()
        )
    })?;
    let modified_unix_nanos = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or_default();
    Ok(ProviderOwnerMetadata {
        file_identity: format!("path:{}", path.display()),
        size_bytes: metadata.len(),
        modified_unix_nanos,
        change_time_unix_nanos: modified_unix_nanos,
    })
}

fn resolve_incremental_owner_state(
    owner_project_root: &Path,
    context: &SearchOwnerItemsFastContext<'_>,
) -> Result<Option<ProviderIncrementalScoped>, String> {
    let Some(provider_context) = context.provider_context else {
        return Ok(None);
    };
    let resolved =
        agent_semantic_client_core::state_core::ResolvedState::resolve(owner_project_root)?;
    let provider_workspace =
        agent_semantic_client::source_index::provider_workspace_identity_v1(owner_project_root)?;
    let scope = ProviderIncrementalScoped {
        project_root: resolved.workspace.root.to_string_lossy().into_owned(),
        workspace_identity: resolved.workspace.workspace_id.to_string(),
        provider_workspace_identity_digest: provider_workspace.digest,
        language_id: context.language_id.to_owned(),
        provider_id: provider_context.provider.provider_id.as_str().to_owned(),
        provider_workspace_root: provider_workspace.root,
    };
    Ok(Some(scope))
}

fn emit_source_index_trace(_state: &OwnerItemsSearchState<'_>) -> Result<(), String> {
    Ok(())
}

pub(super) fn run_search_owner_items_query_command(
    args: &[String],
    context: SearchOwnerItemsFastContext<'_>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(context.frontier_receipt)?;
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if !matches!(owner_query_args.view.as_str(), "seeds" | "hits") {
        return Err(
            "search owner items fast path supports --view seeds or --view hits".to_string(),
        );
    }
    let owner_project_root = language_owner_items_workspace_root(
        context.project_root,
        context.locator_root,
        search_owner_items_workspace(args).as_deref(),
    );
    let state = OwnerItemsSearchState::new(
        context,
        &owner_query_args.owner,
        owner_query_args.query.as_str(),
        owner_project_root,
    )?;
    emit_source_index_trace(&state)?;
    state.run_provider()
}

fn search_owner_items_workspace(args: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--workspace" {
            return args.get(index + 1).map(PathBuf::from);
        }
        index += 1;
    }
    None
}

#[cfg(test)]
#[path = "../../tests/unit/search_pipe_incremental_owner_warm.rs"]
mod tests;
