//! Manifest-first dependency facts for ASP-owned search pipe graph requests.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use serde_json::Value as JsonValue;
use toml::Value;

use super::search_pipe_model::Candidate;

const DEPENDENCY_FACT_LIMIT: usize = 16;

#[derive(Debug, Clone)]
pub(super) struct DependencyFact {
    pub(super) owner_path: String,
    pub(super) dependency: String,
    pub(super) version: Option<String>,
    pub(super) source: &'static str,
}

pub(super) fn collect_dependency_facts(
    language_id: &str,
    project_root: &Path,
    query: Option<&str>,
    candidates: &[Candidate],
) -> Vec<DependencyFact> {
    let mut seen_facts = HashSet::new();
    let mut facts = Vec::new();
    push_manifest_dependency_facts(language_id, project_root, &mut seen_facts, &mut facts);
    for candidate in candidates {
        push_dependency_fact(
            language_id,
            &candidate.path,
            &candidate.text,
            None,
            "usage",
            &mut seen_facts,
            &mut facts,
        );
        if facts.len() >= DEPENDENCY_FACT_LIMIT {
            break;
        }
    }
    if let Some(query) = query {
        facts.sort_by_key(|fact| dependency_fact_rank(fact, query));
        if facts
            .iter()
            .any(|fact| dependency_matches_query(&fact.dependency, query))
        {
            facts.retain(|fact| dependency_matches_query(&fact.dependency, query));
        }
    }
    facts.truncate(DEPENDENCY_FACT_LIMIT);
    facts
}

pub(super) fn dependency_matches_query(dependency: &str, query: &str) -> bool {
    let dependency = dependency.to_ascii_lowercase();
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .any(|term| {
            dependency == term
                || dependency.contains(&term)
                || term.starts_with(&format!("{dependency}::"))
                || term.starts_with(&format!("{dependency}@"))
        })
}

fn push_manifest_dependency_facts(
    language_id: &str,
    project_root: &Path,
    seen_facts: &mut HashSet<String>,
    facts: &mut Vec<DependencyFact>,
) {
    match language_id {
        "rust" => {
            let lock_versions = cargo_lock_versions(project_root);
            push_manifest_dependency_rows(
                "Cargo.toml",
                cargo_manifest_dependencies(project_root, &lock_versions),
                seen_facts,
                facts,
            );
        }
        "typescript" => push_manifest_dependency_rows(
            "package.json",
            package_json_dependencies(project_root),
            seen_facts,
            facts,
        ),
        "python" => push_manifest_dependency_rows(
            "pyproject.toml",
            pyproject_dependencies(project_root),
            seen_facts,
            facts,
        ),
        "julia" => push_manifest_dependency_rows(
            "Project.toml",
            julia_project_dependencies(project_root),
            seen_facts,
            facts,
        ),
        "gerbil-scheme" => push_manifest_dependency_rows(
            "gerbil.pkg",
            gerbil_pkg_dependencies(project_root),
            seen_facts,
            facts,
        ),
        _ => {}
    }
}

fn push_manifest_dependency_rows(
    owner_path: &str,
    dependencies: Vec<(String, Option<String>)>,
    seen_facts: &mut HashSet<String>,
    facts: &mut Vec<DependencyFact>,
) {
    for (dependency, version) in dependencies {
        let key = format!("{owner_path}:{dependency}:manifest");
        if seen_facts.insert(key) {
            facts.push(DependencyFact {
                owner_path: owner_path.to_string(),
                dependency,
                version,
                source: "manifest",
            });
        }
    }
}

fn dependency_fact_rank(fact: &DependencyFact, query: &str) -> (bool, bool) {
    (
        !dependency_matches_query(&fact.dependency, query),
        fact.source != "manifest",
    )
}

fn push_dependency_fact(
    language_id: &str,
    owner_path: &str,
    line: &str,
    version: Option<String>,
    source: &'static str,
    seen_facts: &mut HashSet<String>,
    facts: &mut Vec<DependencyFact>,
) {
    let Some(dependency) = dependency_from_line(language_id, line) else {
        return;
    };
    let key = format!("{owner_path}:{dependency}:{source}");
    if seen_facts.insert(key) {
        facts.push(DependencyFact {
            owner_path: owner_path.to_string(),
            dependency,
            version,
            source,
        });
    }
}

fn cargo_manifest_dependencies(
    project_root: &Path,
    lock_versions: &std::collections::HashMap<String, String>,
) -> Vec<(String, Option<String>)> {
    let Ok(text) = fs::read_to_string(project_root.join("Cargo.toml")) else {
        return Vec::new();
    };
    let Ok(value) = toml::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    for table_path in [
        &["dependencies"][..],
        &["dev-dependencies"][..],
        &["build-dependencies"][..],
        &["workspace", "dependencies"][..],
    ] {
        let Some(table) = toml_table(&value, table_path) else {
            continue;
        };
        dependencies.extend(table.iter().map(|(name, spec)| {
            let version = lock_versions
                .get(&normalize_dependency_name(name))
                .cloned()
                .or_else(|| dependency_version(spec));
            (name.clone(), version)
        }));
    }
    dependencies.sort_by(|left, right| left.0.cmp(&right.0));
    dependencies.dedup_by(|left, right| left.0 == right.0);
    dependencies
}

fn cargo_lock_versions(project_root: &Path) -> std::collections::HashMap<String, String> {
    let Ok(text) = fs::read_to_string(project_root.join("Cargo.lock")) else {
        return std::collections::HashMap::new();
    };
    let Ok(value) = toml::from_str::<Value>(&text) else {
        return std::collections::HashMap::new();
    };
    value
        .get("package")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let version = entry.get("version")?.as_str()?;
            Some((normalize_dependency_name(name), version.to_string()))
        })
        .collect()
}

fn package_json_dependencies(project_root: &Path) -> Vec<(String, Option<String>)> {
    let Ok(text) = fs::read_to_string(project_root.join("package.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<JsonValue>(&text) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        dependencies.extend(
            value
                .get(key)
                .and_then(JsonValue::as_object)
                .into_iter()
                .flat_map(|table| {
                    table
                        .iter()
                        .map(|(name, spec)| (name.clone(), json_dependency_version(spec)))
                }),
        );
    }
    sort_dedup_dependencies(&mut dependencies);
    dependencies
}

fn pyproject_dependencies(project_root: &Path) -> Vec<(String, Option<String>)> {
    let Ok(text) = fs::read_to_string(project_root.join("pyproject.toml")) else {
        return Vec::new();
    };
    let Ok(value) = toml::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    if let Some(project) = value.get("project") {
        dependencies.extend(
            project
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(python_dependency_spec),
        );
        dependencies.extend(
            project
                .get("optional-dependencies")
                .and_then(Value::as_table)
                .into_iter()
                .flat_map(|groups| groups.values())
                .filter_map(Value::as_array)
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(python_dependency_spec),
        );
    }
    if let Some(poetry) = toml_table(&value, &["tool", "poetry", "dependencies"]) {
        dependencies.extend(
            poetry
                .iter()
                .filter(|(name, _)| name.as_str() != "python")
                .map(|(name, spec)| (name.clone(), dependency_version(spec))),
        );
    }
    sort_dedup_dependencies(&mut dependencies);
    dependencies
}

fn julia_project_dependencies(project_root: &Path) -> Vec<(String, Option<String>)> {
    let Ok(text) = fs::read_to_string(project_root.join("Project.toml")) else {
        return Vec::new();
    };
    let manifest_versions = julia_manifest_versions(project_root);
    let compat_versions = julia_project_compat_versions(&text);
    let mut dependencies = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_deps = trimmed == "[deps]";
            continue;
        }
        if !in_deps {
            continue;
        }
        let Some((name, _)) = toml_key_value(trimmed) else {
            continue;
        };
        let version = manifest_versions
            .get(&name)
            .cloned()
            .or_else(|| compat_versions.get(&name).cloned());
        dependencies.push((name, version));
    }
    sort_dedup_dependencies(&mut dependencies);
    dependencies
}

fn julia_project_compat_versions(text: &str) -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let mut in_compat = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_compat = trimmed == "[compat]";
            continue;
        }
        if !in_compat {
            continue;
        }
        if let Some((name, value)) = toml_key_value(trimmed)
            && let Some(version) = toml_string_value(value)
        {
            versions.insert(name, version);
        }
    }
    versions
}

fn julia_manifest_versions(project_root: &Path) -> HashMap<String, String> {
    let Ok(text) = fs::read_to_string(project_root.join("Manifest.toml")) else {
        return HashMap::new();
    };
    let mut versions = HashMap::new();
    let mut dependency_name = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = julia_manifest_dependency_header(trimmed) {
            dependency_name = Some(name);
            continue;
        }
        let Some(name) = dependency_name.as_ref() else {
            continue;
        };
        let Some((key, value)) = toml_key_value(trimmed) else {
            continue;
        };
        if key == "version"
            && let Some(version) = toml_string_value(value)
        {
            versions.insert(name.clone(), version);
        }
    }
    versions
}

fn julia_manifest_dependency_header(line: &str) -> Option<String> {
    line.strip_prefix("[[deps.")
        .and_then(|rest| rest.strip_suffix("]]"))
        .or_else(|| {
            line.strip_prefix("[deps.")
                .and_then(|rest| rest.strip_suffix(']'))
        })
        .map(trim_toml_key)
        .filter(|name| !name.is_empty())
}

fn toml_key_value(line: &str) -> Option<(String, &str)> {
    let line = line.split('#').next().unwrap_or(line).trim();
    let (key, value) = line.split_once('=')?;
    Some((trim_toml_key(key), value.trim()))
}

fn trim_toml_key(key: &str) -> String {
    key.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn toml_string_value(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(unquoted) = value
        .strip_prefix('"')
        .and_then(|rest| rest.split('"').next())
    {
        return Some(unquoted.to_string()).filter(|version| version != "*");
    }
    value
        .strip_prefix('\'')
        .and_then(|rest| rest.split('\'').next())
        .map(ToOwned::to_owned)
        .filter(|version| version != "*")
}

fn gerbil_pkg_dependencies(project_root: &Path) -> Vec<(String, Option<String>)> {
    let Ok(text) = fs::read_to_string(project_root.join("gerbil.pkg")) else {
        return Vec::new();
    };
    let mut dependencies = text
        .lines()
        .filter(|line| line.contains("depend:"))
        .flat_map(quoted_dependencies)
        .map(|dependency| (dependency, None))
        .collect::<Vec<_>>();
    sort_dedup_dependencies(&mut dependencies);
    dependencies
}

fn toml_table<'a>(value: &'a Value, path: &[&str]) -> Option<&'a toml::map::Map<String, Value>> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))?
        .as_table()
}

fn dependency_version(spec: &Value) -> Option<String> {
    if let Some(version) = spec.as_str() {
        return Some(version.to_string()).filter(|version| version != "*");
    }
    spec.get("version")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn json_dependency_version(spec: &JsonValue) -> Option<String> {
    spec.as_str()
        .map(ToOwned::to_owned)
        .filter(|version| version != "*")
}

fn python_dependency_spec(spec: &str) -> Option<(String, Option<String>)> {
    let name_end = spec
        .find(|character: char| {
            !(character == '_'
                || character == '-'
                || character == '.'
                || character.is_ascii_alphanumeric())
        })
        .unwrap_or(spec.len());
    let name = spec[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    let version = spec[name_end..].trim();
    Some((
        name.to_string(),
        (!version.is_empty()).then(|| version.to_string()),
    ))
}

fn sort_dedup_dependencies(dependencies: &mut Vec<(String, Option<String>)>) {
    dependencies.sort_by(|left, right| left.0.cmp(&right.0));
    dependencies.dedup_by(|left, right| left.0 == right.0);
}

fn normalize_dependency_name(name: &str) -> String {
    name.replace(['_', '.'], "-").to_ascii_lowercase()
}

fn dependency_from_line(language_id: &str, line: &str) -> Option<String> {
    match language_id {
        "rust" => rust_dependency_from_line(line),
        "typescript" => typescript_dependency_from_line(line),
        "python" => python_dependency_from_line(line),
        "julia" => julia_dependency_from_line(line),
        "gerbil-scheme" => gerbil_dependency_from_line(line),
        _ => None,
    }
}

fn rust_dependency_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let import = trimmed.strip_prefix("use ")?;
    Some(import_root(import.split("::").next().unwrap_or(import)))
}

fn typescript_dependency_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("import ") || trimmed.contains("require(") {
        quoted_dependency(trimmed)
    } else {
        None
    }
}

fn python_dependency_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if let Some(import) = trimmed.strip_prefix("import ") {
        return Some(import_root(import));
    }
    trimmed.strip_prefix("from ").map(import_root)
}

fn julia_dependency_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("using ")
        .or_else(|| trimmed.strip_prefix("import "))
        .map(import_root)
}

fn gerbil_dependency_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.contains("depend:") {
        return quoted_dependencies(trimmed).into_iter().next();
    }
    let import = trimmed
        .strip_prefix("(import ")
        .or_else(|| trimmed.strip_prefix("import "))
        .unwrap_or(trimmed);
    let module = import
        .split_whitespace()
        .find(|part| part.starts_with(':'))?;
    Some(import_root(module.trim_start_matches(':')))
}

fn quoted_dependency(line: &str) -> Option<String> {
    let start = line.find('"').or_else(|| line.find('\''))?;
    let quote = line.as_bytes()[start] as char;
    let rest = &line[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn quoted_dependencies(line: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"').or_else(|| rest.find('\'')) {
        let quote = rest.as_bytes()[start] as char;
        let quoted = &rest[start + 1..];
        let Some(end) = quoted.find(quote) else {
            break;
        };
        dependencies.push(quoted[..end].to_string());
        rest = &quoted[end + 1..];
    }
    dependencies
}

fn import_root(import: &str) -> String {
    import
        .split(|character: char| {
            !(character == '_'
                || character == '-'
                || character == ':'
                || character.is_ascii_alphanumeric())
        })
        .find(|part| !part.is_empty())
        .unwrap_or("dependency")
        .to_string()
}
