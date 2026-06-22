//! ASP-owned manifest dependency seed renderer.

use std::path::Path;

use super::search_config::AspConfig;
use super::search_pipe_dependency_facts::{DependencyFact, dependency_matches_query};
use super::search_pipe_dependency_seed_cache::collect_cached_manifest_dependency_facts;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;

pub(super) fn is_search_dependency_seed(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("deps" | "dependency"))
        && dependency_seed_query(args).is_some()
        && !args.iter().any(|arg| arg == "--json" || arg == "--code")
}

pub(super) fn run_search_dependency_seed_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    cache_home: &Path,
    config: &AspConfig,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<(), String> {
    let query = dependency_seed_query(args)
        .ok_or_else(|| "search deps requires a dependency query".to_string())?;
    let view = explicit_view(args).unwrap_or("hits");
    let seed = collect_cached_manifest_dependency_facts(
        language_id,
        project_root,
        cache_home,
        config,
        provider_context,
    );
    let facts = seed
        .facts
        .into_iter()
        .filter(|fact| dependency_matches_query(&fact.dependency, query))
        .collect::<Vec<_>>();
    render_dependency_seed(
        language_id,
        query,
        view,
        seed.cache_status,
        seed.topology_source,
        &facts,
    );
    Ok(())
}

fn render_dependency_seed(
    language_id: &str,
    query: &str,
    view: &str,
    seed_cache: &str,
    topology: &str,
    facts: &[DependencyFact],
) {
    println!(
        "[search-deps] lang={language_id} q={query} manifest={} usage=0 topology={topology} seedCache={seed_cache} hit={} view={view}",
        facts.len(),
        facts.len()
    );
    for fact in facts {
        println!(
            "|dependency D:{} requirement=\"{}\" source={} owner={} versionScope=current",
            fact.dependency,
            fact.version.as_deref().unwrap_or("-"),
            fact.source,
            fact.owner_path
        );
        println!(
            "|hit path={} owner=. kind=dependency score=10 reason=manifest-package-exact dependency={} versionScope=current",
            fact.owner_path, fact.dependency
        );
    }
    println!(
        "|note kind=fact-scope message=\"deps view exposes provider dependency topology when available; ASP parser fallback is compatibility only\""
    );
    println!("|next dependency:{query},public-external-types:{query}");
}

fn dependency_seed_query(args: &[String]) -> Option<&str> {
    if !matches!(args.first().map(String::as_str), Some("search"))
        || !matches!(args.get(1).map(String::as_str), Some("deps" | "dependency"))
    {
        return None;
    }
    let mut index = 2;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == "--workspace" || arg == "--view" {
            index += 2;
            continue;
        }
        if arg.starts_with("--workspace=") || arg.starts_with("--view=") {
            index += 1;
            continue;
        }
        if arg.starts_with('-') {
            index += 1;
            continue;
        }
        return Some(arg);
    }
    None
}

fn explicit_view(args: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--view" {
            return args.get(index + 1).map(String::as_str);
        }
        if let Some(value) = args[index].strip_prefix("--view=") {
            return Some(value);
        }
        index += 1;
    }
    None
}
