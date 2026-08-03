//! Runtime Server-backed `search owner <path> items` adapter.

use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
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
    let session = super::runtime_server::block_on_agent_facing_runtime_server_client(
        tokio::time::Instant::now(),
        "search",
        "resident-owner-freshness-session",
        super::runtime_server::runtime_server_workspace_session_for_admission_async(&project_root),
    )?;
    let freshness = super::runtime_server::block_on_agent_facing_runtime_server_client(
        tokio::time::Instant::now(),
        "search",
        "resident-owner-freshness-ensure",
        session.ensure_runtime_owner(context.language_id, &owner_path),
    )?;
    freshness.validate()?;
    let client = super::runtime_server::block_on_agent_facing_runtime_server_client(
        tokio::time::Instant::now(),
        "search",
        "resident-exact-generation-open",
        super::runtime_server::runtime_server_workspace_exact_projection_client_async(
            &project_root,
        ),
    )?;
    // The CLI remains a pure client: freshness mutation is serialized by the
    // workspace resident writer lane before this mmap read is opened.
    let provider_invocations = 0;
    let owner = client
        .owner_snapshot(&owner_path)?
        .ok_or_else(|| {
            format!(
                "owner search state=owner-missing reasonKind=owner-not-in-workspace ownerPath={owner_path} rootDigest={}",
                client.root_digest()
            )
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
        freshness.changed,
        provider_invocations,
        2
    );
    let _ = context.started;
    let _ = context.provider_context;
    Ok(())
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
