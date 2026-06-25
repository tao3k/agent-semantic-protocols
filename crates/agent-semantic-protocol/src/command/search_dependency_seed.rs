//! ASP-owned manifest dependency seed renderer.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::search_config::AspConfig;
use super::search_pipe_dependency_facts::{DependencyFact, dependency_matches_query};
use super::search_pipe_dependency_seed_cache::collect_cached_manifest_dependency_facts;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;

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
    );
    let facts = seed
        .facts
        .into_iter()
        .filter(|fact| dependency_matches_query(&fact.dependency, query.raw))
        .collect::<Vec<_>>();
    render_dependency_seed(
        language_id,
        &query,
        view,
        project_root,
        seed.cache_status,
        seed.topology_source,
        &facts,
    );
    Ok(())
}

fn render_dependency_seed(
    language_id: &str,
    query: &DependencySeedSelector<'_>,
    view: &str,
    project_root: &Path,
    seed_cache: &str,
    topology: &str,
    facts: &[DependencyFact],
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
    if language_id == "rust"
        && (query.api.is_some() || view == "public-external-types")
        && let Some(external_api) = resolve_rust_external_api(project_root, query)
    {
        render_rust_external_api(&external_api, query);
    }
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

#[derive(Debug)]
struct RustExternalApi {
    package: String,
    manifest_path: PathBuf,
    source_root: PathBuf,
    items: Vec<RustExternalApiItem>,
}

#[derive(Debug)]
struct RustExternalApiItem {
    name: String,
    kind: &'static str,
    path: PathBuf,
    line: usize,
}

fn resolve_rust_external_api(
    project_root: &Path,
    query: &DependencySeedSelector<'_>,
) -> Option<RustExternalApi> {
    let package = rust_dependency_package_from_metadata(project_root, query.dependency)?;
    let items = collect_rust_public_api_items(&package.source_root, &package.lib_path, query.api);
    Some(RustExternalApi {
        package: package.name,
        manifest_path: package.manifest_path,
        source_root: package.source_root,
        items,
    })
}

fn render_rust_external_api(external_api: &RustExternalApi, query: &DependencySeedSelector<'_>) {
    println!(
        "|external dependency={} package={} source=cargo-metadata manifest={} sourceRoot={}",
        query.dependency,
        external_api.package,
        external_api.manifest_path.display(),
        external_api.source_root.display()
    );
    if external_api.items.is_empty() {
        let api = query.api.unwrap_or("*");
        println!(
            "|external-api status=miss dependency={} api={} source=cargo-metadata next=revise-api-or-run-crate-source",
            query.dependency, api
        );
        return;
    }
    for item in external_api.items.iter().take(16) {
        let selector = format!("{}:{}-{}", item.path.display(), item.line, item.line);
        println!(
            "|external-api name={} kind={} path={} selector={} source=cargo-metadata match={}",
            item.name,
            item.kind,
            item.path.display(),
            selector,
            external_api_match_kind(query.api, &item.name)
        );
        println!(
            "|next public-external-types:{}::{} query=\"asp rust query --selector {} --workspace {} --code\"",
            query.dependency,
            item.name,
            selector,
            external_api.source_root.display()
        );
    }
}

fn external_api_match_kind(api: Option<&str>, item_name: &str) -> &'static str {
    let Some(api) = api else {
        return "frontier";
    };
    if item_name == api {
        "exact"
    } else if item_name.eq_ignore_ascii_case(api) {
        "case-insensitive"
    } else {
        "contains"
    }
}

struct RustDependencyPackage {
    name: String,
    manifest_path: PathBuf,
    source_root: PathBuf,
    lib_path: PathBuf,
}

fn rust_dependency_package_from_metadata(
    project_root: &Path,
    dependency: &str,
) -> Option<RustDependencyPackage> {
    if let Some(package) = rust_path_dependency_package_from_manifest(project_root, dependency) {
        return Some(package);
    }
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--offline"])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
    let packages = value.get("packages")?.as_array()?;
    let package = packages.iter().find(|package| {
        package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| rust_dependency_name_matches(name, dependency))
    })?;
    let name = package.get("name")?.as_str()?.to_string();
    let manifest_path = PathBuf::from(package.get("manifest_path")?.as_str()?);
    let lib_path = package
        .get("targets")?
        .as_array()?
        .iter()
        .find(|target| {
            target
                .get("kind")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .any(|kind| kind.as_str() == Some("lib"))
        })?
        .get("src_path")?
        .as_str()
        .map(PathBuf::from)?;
    let source_root = lib_path.parent()?.to_path_buf();
    Some(RustDependencyPackage {
        name,
        manifest_path,
        source_root,
        lib_path,
    })
}

fn rust_path_dependency_package_from_manifest(
    project_root: &Path,
    dependency: &str,
) -> Option<RustDependencyPackage> {
    let manifest_path = project_root.join("Cargo.toml");
    let manifest = read_toml_value(&manifest_path)?;
    let dependency_path = path_dependency_value(&manifest, dependency)?;
    let dependency_root = if dependency_path.is_absolute() {
        dependency_path
    } else {
        project_root.join(dependency_path)
    };
    let dependency_manifest_path = dependency_root.join("Cargo.toml");
    let dependency_manifest = read_toml_value(&dependency_manifest_path)?;
    let name = dependency_manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(dependency)
        .to_string();
    if !rust_dependency_name_matches(&name, dependency) {
        return None;
    }
    let lib_path = dependency_manifest
        .get("lib")
        .and_then(toml::Value::as_table)
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .map_or_else(
            || dependency_root.join("src/lib.rs"),
            |path| dependency_root.join(path),
        );
    let source_root = lib_path.parent()?.to_path_buf();
    Some(RustDependencyPackage {
        name,
        manifest_path: dependency_manifest_path,
        source_root,
        lib_path,
    })
}

fn read_toml_value(path: &Path) -> Option<toml::Value> {
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn path_dependency_value(manifest: &toml::Value, dependency: &str) -> Option<PathBuf> {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(path) = path_dependency_value_in_section(manifest, section, dependency) else {
            continue;
        };
        return Some(path);
    }
    None
}

fn path_dependency_value_in_section(
    manifest: &toml::Value,
    section: &str,
    dependency: &str,
) -> Option<PathBuf> {
    let dependencies = manifest.get(section)?.as_table()?;
    for (name, spec) in dependencies {
        if !rust_dependency_name_matches(name, dependency) {
            continue;
        }
        let path = spec
            .as_table()
            .and_then(|table| table.get("path"))
            .and_then(toml::Value::as_str)?;
        return Some(PathBuf::from(path));
    }
    None
}

fn rust_dependency_name_matches(package: &str, dependency: &str) -> bool {
    package == dependency || package.replace('-', "_") == dependency.replace('-', "_")
}

fn collect_rust_public_api_items(
    source_root: &Path,
    lib_path: &Path,
    api: Option<&str>,
) -> Vec<RustExternalApiItem> {
    let mut visited = std::collections::HashSet::new();
    let mut items = Vec::new();
    collect_rust_public_api_items_from_file(source_root, lib_path, api, &mut visited, &mut items);
    items.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.name.cmp(&right.name))
    });
    items
}

fn collect_rust_public_api_items_from_file(
    source_root: &Path,
    path: &Path,
    api: Option<&str>,
    visited: &mut std::collections::HashSet<PathBuf>,
    items: &mut Vec<RustExternalApiItem>,
) {
    let path = path.to_path_buf();
    if !visited.insert(path.clone()) {
        return;
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(file) = syn::parse_file(&text) else {
        return;
    };
    for item in file.items {
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "struct", item.ident.to_string(), &path, &text);
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "enum", item.ident.to_string(), &path, &text);
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "trait", item.ident.to_string(), &path, &text);
            }
            syn::Item::Type(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "type", item.ident.to_string(), &path, &text);
            }
            syn::Item::Fn(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "fn", item.sig.ident.to_string(), &path, &text);
            }
            syn::Item::Const(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "const", item.ident.to_string(), &path, &text);
            }
            syn::Item::Static(item) if is_public(&item.vis) => {
                push_rust_api_item(api, items, "static", item.ident.to_string(), &path, &text);
            }
            syn::Item::Use(item) if is_public(&item.vis) => {
                push_rust_use_tree_api_items(api, items, &item.tree, &path, &text);
            }
            syn::Item::Mod(item) if is_public(&item.vis) && item.content.is_none() => {
                if let Some(module_path) = rust_module_file(source_root, &item.ident.to_string()) {
                    collect_rust_public_api_items_from_file(
                        source_root,
                        &module_path,
                        api,
                        visited,
                        items,
                    );
                }
            }
            _ => {}
        }
    }
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn push_rust_api_item(
    api: Option<&str>,
    items: &mut Vec<RustExternalApiItem>,
    kind: &'static str,
    name: String,
    path: &Path,
    text: &str,
) {
    if !api_matches_item(api, &name) {
        return;
    }
    let line = rust_item_line(text, &name).unwrap_or(1);
    items.push(RustExternalApiItem {
        name,
        kind,
        path: path.to_path_buf(),
        line,
    });
}

fn push_rust_use_tree_api_items(
    api: Option<&str>,
    items: &mut Vec<RustExternalApiItem>,
    tree: &syn::UseTree,
    path: &Path,
    text: &str,
) {
    match tree {
        syn::UseTree::Name(name) => {
            push_rust_api_item(api, items, "use", name.ident.to_string(), path, text);
        }
        syn::UseTree::Rename(rename) => {
            push_rust_api_item(api, items, "use", rename.rename.to_string(), path, text);
        }
        syn::UseTree::Path(path_tree) => {
            push_rust_use_tree_api_items(api, items, &path_tree.tree, path, text);
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                push_rust_use_tree_api_items(api, items, tree, path, text);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn api_matches_item(api: Option<&str>, name: &str) -> bool {
    let Some(api) = api else {
        return true;
    };
    let api = api.trim();
    name == api || name.eq_ignore_ascii_case(api) || name.contains(api)
}

fn rust_item_line(text: &str, name: &str) -> Option<usize> {
    text.lines()
        .position(|line| line_contains_rust_item_name(line, name))
        .map(|index| index + 1)
}

fn line_contains_rust_item_name(line: &str, name: &str) -> bool {
    let line = line.trim_start();
    if !line.starts_with("pub ") && !line.starts_with("pub(") {
        return false;
    }
    line.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .any(|token| token == name)
}

fn rust_module_file(source_root: &Path, module: &str) -> Option<PathBuf> {
    let direct = source_root.join(format!("{module}.rs"));
    if direct.exists() {
        return Some(direct);
    }
    let nested = source_root.join(module).join("mod.rs");
    nested.exists().then_some(nested)
}

struct DependencySeedSelector<'a> {
    raw: &'a str,
    dependency: &'a str,
    api: Option<&'a str>,
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
    if !extra.is_empty() {
        let suggestion = if raw.contains("::") {
            raw.to_string()
        } else {
            format!("{raw}::{}", extra.join("::"))
        };
        return Err(format!(
            "search deps accepts one dependency selector; unexpected extra argument '{}'. Use `search deps {suggestion}` for API queries.",
            extra[0]
        ));
    }
    let (dependency_part, api) = raw
        .split_once("::")
        .map_or((raw, None), |(dependency, api)| (dependency, Some(api)));
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
