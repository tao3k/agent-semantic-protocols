//! Provider-native `search owner <path> items` command.

use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use agent_semantic_client::language_owner_items_workspace_root;
use agent_semantic_search::search_command_preflight::{
    SearchCommandPreflightOutcome, preflight_search_command_args,
};

#[derive(Debug, Eq, PartialEq)]
struct SearchOwnerItemsArgs {
    owner: PathBuf,
    query: String,
    view: String,
}

pub(super) struct SearchOwnerItemsContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) async fn run_search_owner_items_query_command(
    args: &[String],
    context: SearchOwnerItemsContext<'_>,
) -> Result<(), String> {
    if context.frontier_receipt.is_some() {
        return Err(
            "--frontier-receipt-out is not supported by provider-native owner discovery".to_owned(),
        );
    }
    match preflight_search_command_args(&context.language_id.into(), args, context.project_root) {
        SearchCommandPreflightOutcome::Rejected(error) => return Err(error),
        SearchCommandPreflightOutcome::Passed | SearchCommandPreflightOutcome::NotApplicable => {}
    }
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
    run_server_owner_items(&owner_query_args.query, &project_root, &owner_path, context).await
}

async fn run_server_owner_items(
    query: &str,
    project_root: &Path,
    owner_path: &str,
    context: SearchOwnerItemsContext<'_>,
) -> Result<(), String> {
    let canonical_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.into());
    let session = crate::server::runtime_server::runtime_server_stateless_search_session_async(
        &canonical_root,
    )
    .await?;
    let owner = session
        .project_provider_owner(context.language_id.into(), owner_path)
        .await?;
    let query_alternatives = query
        .split('|')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let selectors = owner
        .selectors
        .into_iter()
        .filter_map(|selector| {
            let identity = agent_semantic_content_identity::CanonicalItemSelector::parse(
                selector.selector.clone(),
            )
            .ok()?;
            let matches = query_alternatives.is_empty()
                || query_alternatives.iter().any(|alternative| {
                    identity
                        .symbol
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(alternative)
                        || identity
                            .kind
                            .as_str()
                            .to_ascii_lowercase()
                            .contains(alternative)
                        || selector.selector.to_ascii_lowercase().contains(alternative)
                        || owner
                            .bytes
                            .get(selector.byte_start..selector.byte_end)
                            .is_some_and(|bytes| {
                                String::from_utf8_lossy(bytes)
                                    .to_ascii_lowercase()
                                    .contains(alternative)
                            })
                });
            matches.then_some((selector, identity))
        })
        .collect::<Vec<_>>();
    println!(
        "[search-owner] q={} owner={} selector=items alg=asp-provider-native-owner-items-v1",
        query, owner_path
    );
    for (selector, identity) in &selectors {
        println!(
            "|item symbol={} kind={} structuralSelector={} reason=provider-native-owner",
            identity.symbol.as_str(),
            identity.kind.as_str(),
            selector.selector
        );
    }
    println!(
        "entries={} generation=provider-native rootDigest={} rootDepth=1,0 ownerChanged=false providerInvocations=1 databaseOpens=0 controlRoundtrips=0",
        selectors.len(),
        owner.content_digest
    );
    Ok(())
}

pub(super) fn is_search_owner_items_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("owner"))
        && matches!(args.get(3).map(String::as_str), Some("items"))
        && !args.iter().any(|arg| arg == "--json")
}

fn parse_search_owner_items_query_args(args: &[String]) -> Result<SearchOwnerItemsArgs, String> {
    let owner = args
        .get(2)
        .filter(|owner| !owner.starts_with('-'))
        .ok_or_else(|| "search owner requires an owner path".to_string())?;
    if args.get(3).map(String::as_str) != Some("items") {
        return Err("search owner requires the `items` projection".to_string());
    }
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 4;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "search owner items --view requires seeds or hits".to_string())?
                    .clone();
                index += 2;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown search owner items option: {option}"));
            }
            argument => {
                return Err(format!(
                    "unexpected search owner items argument: {argument}"
                ));
            }
        }
    }
    Ok(SearchOwnerItemsArgs {
        owner: PathBuf::from(owner),
        query: query.unwrap_or_default(),
        view,
    })
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
#[path = "../../tests/unit/search_owner_items.rs"]
mod tests;
