use std::path::Path;

use crate::command::search_config::AspConfig;

use super::search_pipe_dependency_seed_cache::{
    ProviderDependencyTopologyFact, collect_cached_manifest_dependency_facts,
};
use super::search_pipe_provider_facts::ProviderGraphFactsContext;

struct DependencySeedSelector<'a> {
    raw: &'a str,
    dependency: &'a str,
    api: Option<&'a str>,
}

pub(super) fn is_search_dependency_seed(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("deps" | "dependency"))
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
    let query = dependency_seed_query(args)?;
    let view = explicit_view(args).unwrap_or("hits");
    let seed = collect_cached_manifest_dependency_facts(
        language_id,
        project_root,
        cache_home,
        config,
        provider_context,
    )?;
    let facts = seed
        .facts
        .into_iter()
        .filter(|fact| fact.dependency == query.dependency)
        .collect::<Vec<_>>();
    render_dependency_seed(
        language_id,
        &query,
        view,
        seed.cache_status,
        seed.topology_source,
        &facts,
    );
    Ok(())
}

fn dependency_seed_query(args: &[String]) -> Result<DependencySeedSelector<'_>, String> {
    if !matches!(args.first().map(String::as_str), Some("search"))
        || !matches!(args.get(1).map(String::as_str), Some("deps" | "dependency"))
    {
        return Err("search deps requires a dependency query".to_string());
    }
    let mut selector = None;
    let mut extra = Vec::new();
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
        if selector.is_none() {
            selector = Some(arg);
        } else {
            extra.push(arg);
        }
        index += 1;
    }
    let raw = selector.ok_or_else(|| "search deps requires a dependency query".to_string())?;
    if raw.trim().is_empty() {
        return Err("search deps requires a dependency query".to_string());
    }
    if extra.len() > 1 || (raw.contains("::") && !extra.is_empty()) {
        let suggestion = if raw.contains("::") {
            raw.to_string()
        } else {
            format!("{raw}::{}", extra.join("::"))
        };
        let unexpected = if raw.contains("::") {
            extra[0]
        } else {
            extra[1]
        };
        return Err(format!(
            "search deps accepts one dependency selector plus at most one api term; unexpected extra argument '{unexpected}'. Use `search deps {suggestion}` for API queries."
        ));
    }
    let (dependency_part, inline_api) = raw
        .split_once("::")
        .map_or((raw, None), |(dependency, api)| (dependency, Some(api)));
    let api = inline_api.or_else(|| extra.first().copied());
    let dependency = dependency_part
        .split_once('@')
        .map_or(dependency_part, |(dependency, _)| dependency);
    Ok(DependencySeedSelector {
        raw,
        dependency,
        api: api.filter(|value| !value.is_empty()),
    })
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

fn render_dependency_seed(
    language_id: &str,
    query: &DependencySeedSelector<'_>,
    view: &str,
    seed_cache: &str,
    topology: &str,
    facts: &[ProviderDependencyTopologyFact],
) {
    let mut header = format!(
        "[search-deps] lang={language_id} q={} manifest={} usage=0 topology={topology} seedCache={seed_cache} hit={}",
        query.raw,
        facts.len(),
        facts.len()
    );
    if let Some(api) = query.api {
        header.push_str(" apiQuery=");
        header.push_str(api);
    }
    header.push_str(" view=");
    header.push_str(view);
    println!("{header}");
    for fact in facts {
        println!(
            "|dependency D:{} requirement=\"{}\" source=provider-owned owner={} versionScope=current",
            fact.dependency,
            fact.version.as_deref().unwrap_or("-"),
            fact.owner_path
        );
        println!(
            "|hit path={} owner=. kind=dependency score=10 reason=provider-topology-exact dependency={} versionScope=current",
            fact.owner_path, fact.dependency
        );
    }
    println!(
        "|note kind=fact-scope message=\"deps view exposes activated provider dependency topology\""
    );
    if let Some(api) = query.api {
        println!(
            "|next dependency:{},docs-use:{},crate-source:{},import:{},tests:{api},public-external-types:{}",
            query.raw, query.raw, query.dependency, query.dependency, query.raw
        );
    } else {
        println!(
            "|next dependency:{},docs-use:{},crate-source:{},import:{},public-external-types:{}",
            query.raw, query.raw, query.dependency, query.dependency, query.raw
        );
    }
}
