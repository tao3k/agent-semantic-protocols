//! Runtime Server-backed `search owner <path> items` adapter.

use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_action_frontier::{ActionNode, ActionRoute};
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::language_owner_items_workspace_root;
use agent_semantic_content_identity::CanonicalItemSelector;

pub(super) struct SearchOwnerItemsContext<'a> {
    pub(super) started: tokio::time::Instant,
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn run_search_owner_items_query_command(
    args: &[String],
    context: SearchOwnerItemsContext<'_>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(context.frontier_receipt)?;
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if !matches!(owner_query_args.view.as_str(), "seeds" | "hits") {
        return Err("search owner items supports --view seeds or --view hits".to_owned());
    }
    let project_root = language_owner_items_workspace_root(
        context.project_root,
        context.locator_root,
        search_owner_items_workspace(args).as_deref(),
    );
    let owner_path = normalized_owner_key(&project_root, &owner_query_args.owner)?;
    let (client, owner, project_resolutions, control_roundtrips) =
        crate::server::runtime_server::block_on_agent_facing_runtime_server_client(
            tokio::time::Instant::now(),
            "search",
            "resident-exact-generation-open",
            &project_root,
            async {
                let mut client =
                    crate::server::runtime_server::runtime_server_workspace_exact_projection_client_async(
                        &project_root,
                    )
                    .await?;
                let mut owner = client.owner_snapshot(&owner_path)?;
                let mut project_resolutions = Vec::new();
                let mut control_roundtrips = 0;
                if owner.is_none() {
                    crate::server::runtime_server_generation::ensure_runtime_generation_ready_for_projection_async(
                        &project_root,
                    )
                    .await?;
                    control_roundtrips = 1;
                    client = crate::server::runtime_server::runtime_server_workspace_exact_projection_client_async(
                    &project_root,
                )
                .await?;
                    owner = client.owner_snapshot(&owner_path)?;
                    if owner.is_none() {
                        let generation_client = crate::server::runtime_server::runtime_server_workspace_generation_client_async(
                            &project_root,
                        )
                        .await?;
                        project_resolutions = generation_client
                            .lease()
                            .generation()
                            .project_resolutions
                            .clone();
                    }
                }
                Ok((client, owner, project_resolutions, control_roundtrips))
            },
        )?;
    // The CLI remains a pure client. Filesystem observation and provider
    // projection belong to the watcher and workspace writer lane; this path
    // only reads the admitted immutable MemoryBackend generation.
    let provider_invocations = 0;
    let owner = owner.ok_or_else(|| {
        render_owner_missing_diagnostic(OwnerMissingDiagnosticRequest {
            language_id: context.language_id,
            workspace: search_owner_items_workspace(args)
                .as_deref()
                .and_then(Path::to_str)
                .unwrap_or("."),
            owner_path: &owner_path,
            generation_digest: client.generation_digest(),
            root_digest: client.root_digest(),
            project_resolutions: &project_resolutions,
        })
    })?;
    let query_terms = owner_query_args
        .query
        .split('|')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let items = owner
        .selectors
        .iter()
        .filter_map(|selector| {
            let canonical =
                CanonicalItemSelector::parse_root_or_exact_descendant(&selector.selector).ok()?;
            let symbol = canonical.symbol.as_str().to_ascii_lowercase();
            let kind = canonical.kind.as_str().to_ascii_lowercase();
            let structural_selector = selector.selector.to_ascii_lowercase();
            let item_source_matches = owner
                .bytes
                .get(selector.byte_start..selector.byte_end)
                .is_some_and(|item_bytes| {
                    query_terms.iter().any(|query| {
                        let query = query.as_bytes();
                        item_bytes
                            .windows(query.len())
                            .any(|window| window.eq_ignore_ascii_case(query))
                    })
                });
            let matches = query_terms.is_empty()
                || query_terms.iter().any(|query| {
                    symbol.contains(query)
                        || kind.contains(query)
                        || structural_selector.contains(query)
                })
                || item_source_matches;
            matches.then_some((canonical, selector))
        })
        .collect::<Vec<_>>();

    println!(
        "[search-owner] q={} owner={} selector=items alg=asp-generation-owner-items-v1",
        owner_query_args.query, owner_path
    );
    for (canonical, selector) in &items {
        println!(
            "|item symbol={} kind={} structuralSelector={} reason=canonical-generation-owner",
            canonical.symbol.as_str(),
            canonical.kind.as_str(),
            selector.selector
        );
    }
    println!(
        "entries={} generation={} rootDigest={} rootDepth=1,0 ownerChanged={} providerInvocations={} databaseOpens=0 controlRoundtrips={}",
        items.len(),
        client.generation_digest(),
        client.root_digest(),
        false,
        provider_invocations,
        control_roundtrips
    );
    let _ = context.started;
    let _ = context.provider_context;
    Ok(())
}

struct OwnerMissingDiagnosticRequest<'a> {
    language_id: &'a str,
    workspace: &'a str,
    owner_path: &'a str,
    generation_digest: &'a str,
    root_digest: &'a str,
    project_resolutions: &'a [agent_semantic_runtime::AdmittedProjectResolution],
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnerMissingTopologyPacket {
    schema_id: &'static str,
    schema_version: &'static str,
    state: &'static str,
    reason_kind: &'static str,
    language_id: String,
    owner_path: String,
    generation_digest: String,
    root_digest: String,
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
    action_frontier: Vec<String>,
    recommended_next: String,
}

fn render_owner_missing_diagnostic(request: OwnerMissingDiagnosticRequest<'_>) -> String {
    let topology = agent_semantic_search::graph_owner_missing_topology_projection(
        agent_semantic_search::GraphOwnerMissingTopologyRequest {
            language_id: request.language_id,
            owner_path: request.owner_path,
            generation_digest: request.generation_digest,
            root_digest: request.root_digest,
            project_resolutions: request.project_resolutions,
        },
    );
    let action = ActionNode {
        id: "A1".to_owned(),
        kind: "lexical-owner-evidence".to_owned(),
        suffix: "owner-missing".to_owned(),
        route: ActionRoute::LexicalSearch {
            language_id: request.language_id.to_owned(),
            query: request.owner_path.to_owned(),
            scope: request.workspace.to_owned(),
        },
    };
    let recommended_next = action
        .materialized_command()
        .expect("lexical owner evidence action always materializes");
    let packet = OwnerMissingTopologyPacket {
        schema_id: "agent.semantic-protocols.search-owner-missing-topology",
        schema_version: "1",
        state: "owner-missing",
        reason_kind: "owner-not-in-workspace",
        language_id: request.language_id.to_owned(),
        owner_path: request.owner_path.to_owned(),
        generation_digest: request.generation_digest.to_owned(),
        root_digest: request.root_digest.to_owned(),
        nodes: topology.nodes,
        edges: topology.edges,
        action_frontier: vec![format!("{}.{}", action.id, action.kind)],
        recommended_next,
    };
    let topology = serde_json::to_string(&packet)
        .expect("owner-missing topology contains only serializable typed facts");
    format!(
        "owner search state=owner-missing reasonKind=owner-not-in-workspace ownerPath={} languageId={} generation={} rootDigest={}\nsearchTopology={topology}",
        request.owner_path, request.language_id, request.generation_digest, request.root_digest,
    )
}

fn normalized_owner_key(project_root: &Path, owner: &Path) -> Result<String, String> {
    let relative = if owner.is_absolute() {
        owner.strip_prefix(project_root).map_err(|_| {
            format!(
                "owner {} is outside admitted project root {}",
                owner.display(),
                project_root.display()
            )
        })?
    } else {
        owner
    };
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("owner path must be workspace-relative and non-empty".to_owned());
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn search_owner_items_workspace(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--workspace")
        .map(|pair| PathBuf::from(&pair[1]))
}

#[cfg(test)]
#[path = "../../tests/unit/search_pipe_owner_missing_topology.rs"]
mod owner_missing_topology_tests;
