//! Runtime Server-backed `search owner <path> items` adapter.

use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::language_owner_items_workspace_root;
use agent_semantic_content_identity::CanonicalItemSelector;

pub(super) struct SearchOwnerItemsContext<'a> {
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
    let client = super::runtime_server::block_on_runtime_server_client(
        super::runtime_server::runtime_server_workspace_generation_client_async(&project_root),
    )??;
    // Search is a pure resident read. Provider execution and owner freshness
    // reconciliation belong to the Runtime Server writer lane and must never
    // be triggered by an agent query.
    let provider_invocations = 0;
    let lease = client.lease();
    let generation = lease.generation();
    generation.validate()?;
    let owner = generation
        .owners
        .iter()
        .find(|owner| owner.owner_path == owner_path)
        .ok_or_else(|| {
            format!(
                "owner search state=owner-missing reasonKind=owner-not-in-workspace ownerPath={owner_path} rootDigest={}",
                generation.source_snapshot.root_digest
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
            let matches = query_terms.is_empty()
                || query_terms.iter().any(|query| {
                    symbol.contains(query)
                        || kind.contains(query)
                        || structural_selector.contains(query)
                });
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
        "entries={} generation={} rootDigest={} rootDepth=1,0 providerInvocations={} databaseOpens=0 controlRoundtrips={}",
        items.len(),
        generation.generation_digest,
        generation.source_snapshot.root_digest,
        provider_invocations,
        0
    );
    let _ = context.language_id;
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
