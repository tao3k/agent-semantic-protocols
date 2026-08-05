use std::path::Path;

use agent_semantic_hook::ActivatedProvider;
use serde_json::json;

const MAX_RETAINED_CAPTURES: usize = 256;

#[derive(Debug)]
struct WorkspaceTreeSitterRequest {
    query_source: String,
    json: bool,
}

struct WorkspaceTreeSitterCapture {
    owner_path: String,
    projection: agent_semantic_client_db::ProviderTreeSitterCaptureProjection,
}

pub(super) fn try_run_workspace_tree_sitter_query(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    provider: &ActivatedProvider,
    profiles: &agent_semantic_hook::RuntimeProfiles,
) -> Result<bool, String> {
    let Some(request) = WorkspaceTreeSitterRequest::parse(args)? else {
        return Ok(false);
    };
    let language = agent_semantic_tree_sitter::registered_language_grammar(language_id.into())?;
    let query =
        agent_semantic_tree_sitter::compile_native_query_source(&language, &request.query_source)?;
    if !query.unsupported_predicates().is_empty() {
        return Err(format!(
            "tree-sitter query uses unsupported predicates: {}",
            query.unsupported_predicates().join(",")
        ));
    }
    let result = run_workspace_tree_sitter_query(
        language_id,
        project_root,
        provider,
        profiles,
        &request,
        &language,
        &query,
    )?;
    render_workspace_query(
        language_id,
        &request,
        project_root,
        result.captures,
        result.total_captures,
        &result.read_state,
        &result.receipt,
    )?;
    Ok(true)
}

impl WorkspaceTreeSitterRequest {
    fn parse(args: &[String]) -> Result<Option<Self>, String> {
        if has_exact_selector(args) {
            return Ok(None);
        }
        let Some(query_source) = option_value(args, "--treesitter-query")? else {
            return Ok(None);
        };
        match args.first().map(String::as_str) {
            Some("search") => {}
            Some("query") => {
                return Err(
                    "workspace Tree-sitter discovery is search-owned; use `asp <language> search --treesitter-query <QUERY> --workspace <ROOT>`, or add an exact `--selector` for deterministic query projection"
                        .to_string(),
                );
            }
            _ => return Ok(None),
        }
        Ok(Some(Self {
            query_source,
            json: args.iter().any(|argument| argument == "--json"),
        }))
    }
}

fn has_exact_selector(args: &[String]) -> bool {
    args.iter()
        .any(|argument| argument == "--selector" || argument.starts_with("--selector="))
}

fn option_value(args: &[String], option: &str) -> Result<Option<String>, String> {
    let mut values = args.iter();
    while let Some(argument) = values.next() {
        if argument == option {
            return values
                .next()
                .cloned()
                .map(Some)
                .ok_or_else(|| format!("missing value after {option}"));
        }
        if let Some(value) = argument.strip_prefix(&format!("{option}=")) {
            return Ok(Some(value.to_string()));
        }
    }
    Ok(None)
}

pub(super) fn infer_workspace_tree_sitter_search_language(
    args: &[String],
    _project_root: &Path,
    providers: &[ActivatedProvider],
) -> Result<Option<String>, String> {
    let Some(query_source) = option_value(args, "--treesitter-query")? else {
        return Ok(None);
    };
    let mut compatible_languages = std::collections::BTreeSet::new();

    for provider in providers {
        let language_id = provider.language_id.as_str();
        let Ok(language) =
            agent_semantic_tree_sitter::registered_language_grammar(language_id.into())
        else {
            continue;
        };
        let Ok(query) =
            agent_semantic_tree_sitter::compile_native_query_source(&language, &query_source)
        else {
            continue;
        };
        if !query.unsupported_predicates().is_empty() {
            continue;
        }
        compatible_languages.insert(language_id.to_string());
    }

    match compatible_languages.len() {
        1 => Ok(compatible_languages.into_iter().next()),
        0 => Err(
            "tree-sitter search pattern did not compile for any active language; add `--language <language>` to select the intended grammar"
                .to_string(),
        ),
        _ => Err(format!(
            "tree-sitter search language is ambiguous across active grammars: {}; add `--language <language>`",
            compatible_languages
                .into_iter()
                .collect::<Vec<_>>()
                .join("|"),
        )),
    }
}

const TREE_SITTER_OWNER_BUDGET: u32 = 1;

struct WorkspaceTreeSitterQueryResult {
    captures: Vec<WorkspaceTreeSitterCapture>,
    total_captures: usize,
    read_state: agent_semantic_client_db::ProviderTreeSitterQueryReadState,
    receipt: agent_semantic_client_db::ProviderTreeSitterQueryReceipt,
}

struct TreeSitterQueryState {
    provider_workspace_root: std::path::PathBuf,
    scope: agent_semantic_client_db::ProviderIncrementalScoped,
}

pub(super) struct InventoryOwner {
    pub(super) absolute_path: std::path::PathBuf,
    pub(super) owner_path: String,
    pub(super) metadata: agent_semantic_client_db::ProviderOwnerMetadata,
    pub(super) probe: agent_semantic_client_db::ProviderOwnerProbe,
    pub(super) entry: agent_semantic_client_db::ProviderOwnerInventoryEntry,
}

struct ProcessedOwner {
    owner_path: String,
    fingerprint: agent_semantic_client_db::ProviderOwnerFingerprint,
    source_bytes_read: u64,
    captures: Vec<agent_semantic_client_db::ProviderTreeSitterCaptureProjection>,
}

fn run_workspace_tree_sitter_query(
    language_id: &str,
    project_root: &Path,
    provider: &ActivatedProvider,
    profiles: &agent_semantic_hook::RuntimeProfiles,
    request: &WorkspaceTreeSitterRequest,
    language: &tree_sitter::Language,
    query: &agent_semantic_tree_sitter::CompiledNativeSyntaxQuery,
) -> Result<WorkspaceTreeSitterQueryResult, String> {
    let total_started = std::time::Instant::now();
    let phase_started = std::time::Instant::now();
    let state = resolve_tree_sitter_query_state(language_id, project_root, provider)?;
    let client_db_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create Tree-sitter query runtime: {error}"))?;
    let client_db_session =
        crate::server::runtime_server::runtime_server_workspace_session(project_root)?;
    tree_sitter_trace("resolve-state", phase_started, None);
    let query_identity = agent_semantic_client_db::ProviderTreeSitterQueryIdentity {
        scope: state.scope.clone(),
        query_digest: agent_semantic_content_identity::ArtifactHash::blake3(
            request.query_source.as_bytes(),
        )
        .value,
        capture_names: agent_semantic_tree_sitter::extract_capture_names(&request.query_source),
    };
    let phase_started = std::time::Instant::now();
    let before =
        read_tree_sitter_query(&client_db_runtime, &client_db_session, &query_identity, 0)?;
    tree_sitter_trace("initial-query-read", phase_started, None);
    let phase_started = std::time::Instant::now();
    let mut owners = super::workspace_tree_sitter_inventory::collect_provider_inventory(
        &state.provider_workspace_root,
        provider,
    )?;
    tree_sitter_trace("inventory-enumerate", phase_started, Some(owners.len()));
    let phase_started = std::time::Instant::now();
    probe_inventory_entries(&client_db_runtime, &client_db_session, &state, &mut owners)?;
    tree_sitter_trace("inventory-probe-batch", phase_started, Some(owners.len()));
    if !inventory_matches(before.inventory.as_ref(), owners.as_slice()) {
        let phase_started = std::time::Instant::now();
        publish_inventory(
            &client_db_runtime,
            &client_db_session,
            &state,
            owners.as_slice(),
        )?;
        tree_sitter_trace("inventory-publish", phase_started, Some(owners.len()));
    }
    let phase_started = std::time::Instant::now();
    let scheduled = read_tree_sitter_query(
        &client_db_runtime,
        &client_db_session,
        &query_identity,
        TREE_SITTER_OWNER_BUDGET,
    )?;
    tree_sitter_trace(
        "schedule-query-read",
        phase_started,
        Some(scheduled.scheduled_entries.len()),
    );
    let phase_started = std::time::Instant::now();
    let processed = process_scheduled_owners(
        &client_db_runtime,
        &client_db_session,
        language_id,
        project_root,
        provider,
        profiles,
        language,
        query,
        &state,
        &query_identity,
        scheduled.scheduled_entries.as_slice(),
        &mut owners,
    )?;
    tree_sitter_trace("process-owner", phase_started, Some(processed.len()));
    let processed_owner_count = processed.len().try_into().unwrap_or(u32::MAX);
    let processed_source_bytes = processed
        .iter()
        .map(|owner| owner.source_bytes_read)
        .sum::<u64>();
    if !processed.is_empty() {
        let post_publish_probes = processed
            .iter()
            .map(
                |owner| agent_semantic_client_db::ProviderOwnerBatchProbeRequest {
                    owner_path: owner.owner_path.clone(),
                    metadata: owner.fingerprint.metadata.clone(),
                },
            )
            .collect::<Vec<_>>();
        let phase_started = std::time::Instant::now();
        let generation = publish_inventory(
            &client_db_runtime,
            &client_db_session,
            &state,
            owners.as_slice(),
        )?;
        super::workspace_tree_sitter_query_trace::trace_owner_probe_boundary(
            &client_db_runtime,
            &client_db_session,
            &state.scope,
            post_publish_probes.as_slice(),
            "inventory-publish",
        )?;
        write_processed_query_results(
            &client_db_runtime,
            &client_db_session,
            &query_identity,
            generation.as_str(),
            processed,
        )?;
        super::workspace_tree_sitter_query_trace::trace_owner_probe_boundary(
            &client_db_runtime,
            &client_db_session,
            &state.scope,
            post_publish_probes.as_slice(),
            "query-result-publish",
        )?;
        super::workspace_tree_sitter_query_trace::finish_writes(
            &client_db_runtime,
            &client_db_session,
            &state.scope,
            post_publish_probes.as_slice(),
        )?;
        tree_sitter_trace(
            "owner-results-publish",
            phase_started,
            Some(processed_owner_count as usize),
        );
    }
    let phase_started = std::time::Instant::now();
    let final_read =
        read_tree_sitter_query(&client_db_runtime, &client_db_session, &query_identity, 0)?;
    tree_sitter_trace("final-query-read", phase_started, None);
    let mut receipt = final_read.receipt.ok_or_else(|| {
        "provider Tree-sitter inventory disappeared after publication".to_string()
    })?;
    receipt.scheduled_owner_count = processed_owner_count;
    receipt.counters.source_byte_reads = processed_owner_count;
    receipt.counters.source_bytes_read = processed_source_bytes;
    receipt.counters.provider_parses = processed_owner_count;
    receipt.counters.query_cache_writes = processed_owner_count;
    receipt.counters.complete_owner_refreshes = processed_owner_count;
    receipt.counters.owner_index_writes = processed_owner_count;
    let mut captures = final_read
        .cached_results
        .into_iter()
        .flat_map(|owner| {
            let owner_path = owner.owner_path;
            owner
                .projections
                .into_iter()
                .map(move |projection| WorkspaceTreeSitterCapture {
                    owner_path: owner_path.clone(),
                    projection,
                })
        })
        .collect::<Vec<_>>();
    let total_captures = captures.len();
    captures.truncate(MAX_RETAINED_CAPTURES);
    let result = WorkspaceTreeSitterQueryResult {
        captures,
        total_captures,
        read_state: final_read.state,
        receipt,
    };
    tree_sitter_trace("total", total_started, Some(owners.len()));
    Ok(result)
}

fn resolve_tree_sitter_query_state(
    language_id: &str,
    project_root: &Path,
    provider: &ActivatedProvider,
) -> Result<TreeSitterQueryState, String> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let provider_workspace =
        agent_semantic_client::source_index::provider_workspace_identity_v1(project_root)?;
    let provider_workspace_root = {
        let root = Path::new(provider_workspace.root.as_str());
        if root.is_absolute() {
            root.to_path_buf()
        } else {
            project_root.join(root)
        }
    };
    Ok(TreeSitterQueryState {
        provider_workspace_root,
        scope: agent_semantic_client_db::ProviderIncrementalScoped {
            project_root: resolved.workspace.root.to_string_lossy().into_owned(),
            workspace_identity: resolved.workspace.workspace_id.to_string(),
            provider_workspace_identity_digest: provider_workspace.digest,
            language_id: language_id.to_owned(),
            provider_id: provider.provider_id.as_str().to_owned(),
            provider_workspace_root: provider_workspace.root,
        },
    })
}

fn probe_inventory_entries(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    state: &TreeSitterQueryState,
    owners: &mut [InventoryOwner],
) -> Result<(), String> {
    let requests = owners
        .iter()
        .map(
            |owner| agent_semantic_client_db::ProviderOwnerBatchProbeRequest {
                owner_path: owner.owner_path.clone(),
                metadata: owner.metadata.clone(),
            },
        )
        .collect::<Vec<_>>();
    let probe_receipt = client_db_runtime
        .block_on(client_db_session.probe_provider_owners(&state.scope, requests.as_slice()))?;
    if probe_receipt.results.len() != owners.len() {
        return Err(format!(
            "provider batch probe cardinality drift: owners={} probes={}",
            owners.len(),
            probe_receipt.results.len()
        ));
    }
    owners
        .iter_mut()
        .zip(probe_receipt.results)
        .try_for_each(|(owner, result)| {
            if result.owner_path != owner.owner_path {
                return Err(format!(
                    "provider batch probe owner order drift: expected={} actual={}",
                    owner.owner_path, result.owner_path
                ));
            }
            owner.probe = result.probe;
            let unchanged = owner.probe.decision
                == agent_semantic_client_db::ProviderOwnerDecision::Unchanged
                && owner.probe.content_digest.is_some();
            owner.entry = agent_semantic_client_db::ProviderOwnerInventoryEntry {
                owner_path: owner.owner_path.clone(),
                owner_content_digest: unchanged.then(|| {
                    owner
                        .probe
                        .content_digest
                        .clone()
                        .expect("checked content digest")
                }),
                state: if unchanged {
                    agent_semantic_client_db::ProviderOwnerInventoryEntryState::Indexed
                } else if owner.probe.decision
                    == agent_semantic_client_db::ProviderOwnerDecision::Changed
                {
                    agent_semantic_client_db::ProviderOwnerInventoryEntryState::Dirty
                } else {
                    agent_semantic_client_db::ProviderOwnerInventoryEntryState::Unindexed
                },
            };
            Ok(())
        })
}

fn inventory_matches(
    inventory: Option<&agent_semantic_client_db::ProviderOwnerInventory>,
    owners: &[InventoryOwner],
) -> bool {
    inventory.is_some_and(|inventory| {
        inventory.state == agent_semantic_client_db::ProviderOwnerInventoryState::Exact
            && inventory.entries
                == owners
                    .iter()
                    .map(|owner| owner.entry.clone())
                    .collect::<Vec<_>>()
    })
}

fn publish_inventory(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    state: &TreeSitterQueryState,
    owners: &[InventoryOwner],
) -> Result<String, String> {
    client_db_runtime
        .block_on(client_db_session.upsert_provider_owner_inventory(
            &agent_semantic_client_db::ProviderOwnerInventoryWrite {
                scope: state.scope.clone(),
                state: agent_semantic_client_db::ProviderOwnerInventoryState::Exact,
                entries: owners.iter().map(|owner| owner.entry.clone()).collect(),
            },
        ))
        .map(|receipt| receipt.inventory_generation)
}

fn read_tree_sitter_query(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    query: &agent_semantic_client_db::ProviderTreeSitterQueryIdentity,
    owner_budget: u32,
) -> Result<agent_semantic_client_db::ProviderTreeSitterQueryRead, String> {
    client_db_runtime.block_on(client_db_session.read_provider_treesitter_query(
        query,
        owner_budget,
        None,
    ))
}

fn process_scheduled_owners(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    language_id: &str,
    project_root: &Path,
    provider: &ActivatedProvider,
    profiles: &agent_semantic_hook::RuntimeProfiles,
    language: &tree_sitter::Language,
    query: &agent_semantic_tree_sitter::CompiledNativeSyntaxQuery,
    state: &TreeSitterQueryState,
    query_identity: &agent_semantic_client_db::ProviderTreeSitterQueryIdentity,
    scheduled: &[agent_semantic_client_db::ProviderOwnerInventoryEntry],
    owners: &mut [InventoryOwner],
) -> Result<Vec<ProcessedOwner>, String> {
    scheduled
        .iter()
        .map(|scheduled| {
            let owner = owners
                .iter_mut()
                .find(|owner| owner.owner_path == scheduled.owner_path)
                .ok_or_else(|| {
                    format!(
                        "scheduled provider owner is absent from exact inventory: {}",
                        scheduled.owner_path
                    )
                })?;
            process_owner(
                client_db_runtime,
                client_db_session,
                language_id,
                project_root,
                provider,
                profiles,
                language,
                query,
                state,
                query_identity,
                owner,
            )
        })
        .collect()
}

fn process_owner(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    language_id: &str,
    project_root: &Path,
    provider: &ActivatedProvider,
    profiles: &agent_semantic_hook::RuntimeProfiles,
    language: &tree_sitter::Language,
    query: &agent_semantic_tree_sitter::CompiledNativeSyntaxQuery,
    state: &TreeSitterQueryState,
    _query_identity: &agent_semantic_client_db::ProviderTreeSitterQueryIdentity,
    owner: &mut InventoryOwner,
) -> Result<ProcessedOwner, String> {
    if std::env::var_os("ASP_TREESITTER_TRACE").is_some() {
        eprintln!(
            "[query-treesitter-owner] ownerPath={} decision={:?} sizeBytes={}",
            owner.owner_path, owner.probe.decision, owner.metadata.size_bytes
        );
    }
    let source_bytes = std::fs::read(&owner.absolute_path).map_err(|error| {
        format!(
            "failed to read scheduled provider owner {}: {error}",
            owner.absolute_path.display()
        )
    })?;
    let mut fingerprint = agent_semantic_client_db::ProviderOwnerFingerprint {
        metadata: owner.metadata.clone(),
        content_digest: agent_semantic_content_identity::ArtifactHash::blake3(
            source_bytes.as_slice(),
        )
        .value,
    };
    let owner_projections = super::provider_owner_native::run_provider_owner_native(
        super::provider_owner_native::ProviderOwnerNativeTransportContext {
            language_id,
            provider,
            profiles,
            project_root,
        },
        super::provider_owner_native::ProviderOwnerNativeRequest {
            scope: &state.scope,
            owner_path: owner.owner_path.as_str(),
            fingerprint: &fingerprint,
            source_bytes: source_bytes.as_slice(),
        },
    )?;
    let post_provider_metadata =
        super::provider_owner_native::provider_owner_metadata(owner.absolute_path.as_path())?;
    if post_provider_metadata != fingerprint.metadata {
        let post_provider_source_bytes = std::fs::read(&owner.absolute_path).map_err(|error| {
            format!(
                "failed to verify scheduled provider owner after parse {}: {error}",
                owner.absolute_path.display()
            )
        })?;
        let post_provider_digest = agent_semantic_content_identity::ArtifactHash::blake3(
            post_provider_source_bytes.as_slice(),
        )
        .value;
        if post_provider_digest != fingerprint.content_digest {
            return Err(format!(
                "owner-mutated-during-parse: owner={} beforeDigest={} afterDigest={} beforeSize={} afterSize={}",
                owner.owner_path,
                fingerprint.content_digest,
                post_provider_digest,
                fingerprint.metadata.size_bytes,
                post_provider_metadata.size_bytes,
            ));
        }
        fingerprint.metadata = post_provider_metadata;
    }
    let source = std::str::from_utf8(source_bytes.as_slice()).map_err(|error| {
        format!(
            "Tree-sitter provider owner is not UTF-8: {}: {error}",
            owner.owner_path
        )
    })?;
    let captures = join_capture_projections(
        language,
        query,
        source,
        &owner.owner_path,
        &owner_projections,
    )?;
    client_db_runtime.block_on(client_db_session.write_provider_incremental_owner(
        &agent_semantic_client_db::ProviderIncrementalOwnerWrite {
            scope: state.scope.clone(),
            owner_path: owner.owner_path.clone(),
            fingerprint: fingerprint.clone(),
            source_bytes: source_bytes.clone(),
            projection_completeness: "complete-owner".to_string(),
            projections: owner_projections.clone(),
        },
    ))?;
    owner.entry = agent_semantic_client_db::ProviderOwnerInventoryEntry {
        owner_path: owner.owner_path.clone(),
        owner_content_digest: Some(fingerprint.content_digest.clone()),
        state: agent_semantic_client_db::ProviderOwnerInventoryEntryState::Indexed,
    };
    Ok(ProcessedOwner {
        owner_path: owner.owner_path.clone(),
        fingerprint,
        source_bytes_read: source_bytes.len().try_into().unwrap_or(u64::MAX),
        captures,
    })
}

fn join_capture_projections(
    language: &tree_sitter::Language,
    query: &agent_semantic_tree_sitter::CompiledNativeSyntaxQuery,
    source: &str,
    owner_path: &str,
    owner_projections: &[agent_semantic_client_db::ProviderSelectorProjection],
) -> Result<Vec<agent_semantic_client_db::ProviderTreeSitterCaptureProjection>, String> {
    agent_semantic_tree_sitter::execute_native_query(language, query, source)?
        .matches
        .into_iter()
        .flat_map(|matched| matched.captures)
        .map(|capture| {
            let start = capture.node.start_byte as u64;
            let end = capture.node.end_byte as u64;
            let item = owner_projections
                .iter()
                .filter(|item| item.source_byte_start <= start && end <= item.source_byte_end)
                .min_by_key(|item| item.source_byte_end - item.source_byte_start)
                .ok_or_else(|| {
                    format!(
                        "Tree-sitter capture has no containing complete-owner item: owner={} capture={} span={}..{}",
                        owner_path, capture.capture_name, start, end
                    )
                })?;
            Ok(agent_semantic_client_db::ProviderTreeSitterCaptureProjection {
                structural_selector: item.structural_selector.clone(),
                signature: item.signature.clone(),
                item_kind: item.item_kind.clone(),
                item_name: item.item_name.clone(),
                capture_name: capture.capture_name,
                item_source_byte_start: item.source_byte_start,
                item_source_byte_end: item.source_byte_end,
                source_byte_start: start,
                source_byte_end: end,
            })
        })
        .collect()
}

fn write_processed_query_results(
    client_db_runtime: &tokio::runtime::Runtime,
    client_db_session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    query: &agent_semantic_client_db::ProviderTreeSitterQueryIdentity,
    inventory_generation: &str,
    processed: Vec<ProcessedOwner>,
) -> Result<(), String> {
    processed.into_iter().try_for_each(|owner| {
        client_db_runtime.block_on(client_db_session.write_provider_treesitter_owner_result(
            query,
            &agent_semantic_client_db::ProviderTreeSitterOwnerResult {
                owner_path: owner.owner_path,
                owner_content_digest: owner.fingerprint.content_digest,
                query_digest: query.query_digest.clone(),
                inventory_generation: inventory_generation.to_owned(),
                state: agent_semantic_client_db::ProviderTreeSitterOwnerResultState::Processed,
                complete_owner_refresh_count: 1,
                projections: owner.captures,
            },
        ))?;
        Ok(())
    })
}

fn tree_sitter_trace(phase: &str, started: std::time::Instant, owner_count: Option<usize>) {
    if std::env::var_os("ASP_TREESITTER_TRACE").is_some() {
        eprintln!(
            "[query-treesitter-phase] phase={phase} elapsedMs={:.3} ownerCount={}",
            started.elapsed().as_secs_f64() * 1_000.0,
            owner_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "na".to_string())
        );
    }
}

fn render_tree_sitter_query_summary(
    language_id: &str,
    total_captures: usize,
    retained_captures: usize,
    read_state: &agent_semantic_client_db::ProviderTreeSitterQueryReadState,
    receipt: &agent_semantic_client_db::ProviderTreeSitterQueryReceipt,
) -> String {
    let status = if total_captures == 0 {
        "no-matches"
    } else {
        "matches"
    };
    let state = match read_state {
        agent_semantic_client_db::ProviderTreeSitterQueryReadState::Complete => "complete",
        agent_semantic_client_db::ProviderTreeSitterQueryReadState::Partial
        | agent_semantic_client_db::ProviderTreeSitterQueryReadState::MissingInventory => "partial",
    };
    format!(
        "[search-treesitter] status={status} language={language_id} matches={total_captures} retained={retained_captures} truncated={} state={state} inventory={:?} cachedOwners={} scheduledOwners={} remainingOwners={} remainingKind={:?} sourceReads={} sourceBytes={} providerParses={} queryWrites={} ownerWrites={} fullWalks={} casWrites={} fullMerkle={} unrelatedProviders={}",
        total_captures > retained_captures,
        receipt.inventory_state,
        receipt.cached_owner_count,
        receipt.scheduled_owner_count,
        receipt.remaining_owner_count,
        receipt.remaining_count_kind,
        receipt.counters.source_byte_reads,
        receipt.counters.source_bytes_read,
        receipt.counters.provider_parses,
        receipt.counters.query_cache_writes,
        receipt.counters.owner_index_writes,
        receipt.counters.full_source_walks,
        receipt.counters.cas_writes,
        receipt.counters.full_merkle_rebuilds,
        receipt.counters.unrelated_provider_count,
    )
}

fn render_tree_sitter_query_next(
    language_id: &str,
    request: &WorkspaceTreeSitterRequest,
    project_root: &Path,
    receipt: &agent_semantic_client_db::ProviderTreeSitterQueryReceipt,
) -> Option<String> {
    receipt.continuation.as_ref().map(|continuation| {
        format!(
            "next: continue with `asp {language_id} search --treesitter-query {:?} --workspace {}` nextOwnerCursor={}",
            request.query_source,
            project_root.display(),
            continuation.next_owner_cursor
        )
    })
}

pub(super) fn registered_source_path(owner_path: &str, source_extensions: &[String]) -> bool {
    let Some(extension) = Path::new(owner_path)
        .extension()
        .and_then(|value| value.to_str())
    else {
        return false;
    };
    source_extensions
        .iter()
        .any(|registered| registered.trim_start_matches('.') == extension)
}

fn render_workspace_query(
    language_id: &str,
    request: &WorkspaceTreeSitterRequest,
    project_root: &Path,
    captures: Vec<WorkspaceTreeSitterCapture>,
    total_captures: usize,
    read_state: &agent_semantic_client_db::ProviderTreeSitterQueryReadState,
    receipt: &agent_semantic_client_db::ProviderTreeSitterQueryReceipt,
) -> Result<(), String> {
    let native_fact_refs = captures
        .iter()
        .map(|capture| capture.native_fact_ref(language_id))
        .collect::<Vec<_>>();
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
                "schemaVersion": "1",
                "operation": "search",
                "adapterMode": "native-projection",
                "compatibilityLevel": "native-only",
                "selector": format!("workspace:{}", project_root.display()),
                "nativeFactRefs": native_fact_refs,
                "matchCount": total_captures,
                "retainedMatchCount": captures.len(),
                "truncated": total_captures > captures.len(),
                "cache": { "rawSourceStored": false },
                "execution": {
                    "state": match read_state {
                        agent_semantic_client_db::ProviderTreeSitterQueryReadState::Complete => "complete",
                        agent_semantic_client_db::ProviderTreeSitterQueryReadState::Partial
                        | agent_semantic_client_db::ProviderTreeSitterQueryReadState::MissingInventory => "partial",
                    },
                    "inventory": format!("{:?}", receipt.inventory_state).to_ascii_lowercase(),
                    "cachedOwners": receipt.cached_owner_count,
                    "scheduledOwners": receipt.scheduled_owner_count,
                    "remainingOwners": receipt.remaining_owner_count,
                    "remainingKind": format!("{:?}", receipt.remaining_count_kind).to_ascii_lowercase(),
                    "sourceReads": receipt.counters.source_byte_reads,
                    "sourceBytes": receipt.counters.source_bytes_read,
                    "providerParses": receipt.counters.provider_parses,
                    "queryWrites": receipt.counters.query_cache_writes,
                    "ownerWrites": receipt.counters.owner_index_writes,
                    "fullWalks": receipt.counters.full_source_walks,
                    "casWrites": receipt.counters.cas_writes,
                    "fullMerkle": receipt.counters.full_merkle_rebuilds,
                    "unrelatedProviders": receipt.counters.unrelated_provider_count
                }
            }))
            .map_err(|error| format!("failed to render tree-sitter query JSON: {error}"))?
        );
        return Ok(());
    }
    println!(
        "{}",
        render_tree_sitter_query_summary(
            language_id,
            total_captures,
            captures.len(),
            read_state,
            receipt,
        )
    );
    let continuation_next =
        render_tree_sitter_query_next(language_id, request, project_root, receipt);
    if total_captures == 0 {
        super::tree_sitter_query_diagnostics::render_search_miss_guidance(language_id)
            .iter()
            .filter(|line| continuation_next.is_none() || !line.starts_with("next:"))
            .for_each(|line| println!("{line}"));
        if let Some(next) = continuation_next {
            println!("{next}");
        }
        return Ok(());
    }
    println!(
        "{}",
        super::tree_sitter_query_diagnostics::render_search_match_guidance()
    );
    captures
        .iter()
        .for_each(|capture| println!("{}", capture.compact_line(language_id)));
    println!(
        "{}",
        continuation_next.unwrap_or_else(|| {
            "next: inspect one retained capture with `query --selector <exact-selector> --projection source`."
                .to_string()
        })
    );
    Ok(())
}

impl WorkspaceTreeSitterCapture {
    fn native_fact_ref(&self, language_id: &str) -> String {
        format!(
            "{}:syntax:{}:{}:{}:{}:{}",
            language_id,
            self.owner_path,
            self.projection.source_byte_start,
            self.projection.source_byte_end,
            self.projection.item_kind,
            self.projection.capture_name
        )
    }

    fn compact_line(&self, language_id: &str) -> String {
        format!(
            "I=syntax:{}/{}@{}:{}:{}!code selector={} signature={:?} itemSpan={}..{} nativeFactRef={}",
            self.projection.item_kind,
            self.projection.capture_name,
            self.owner_path,
            self.projection.source_byte_start,
            self.projection.source_byte_end,
            self.projection.structural_selector,
            self.projection.signature,
            self.projection.item_source_byte_start,
            self.projection.item_source_byte_end,
            self.native_fact_ref(language_id)
        )
    }
}

#[cfg(test)]
use super::workspace_tree_sitter_inventory::provider_path_is_ignored;

#[cfg(test)]
#[path = "../../tests/unit/command/workspace_tree_sitter_query.rs"]
mod tests;
