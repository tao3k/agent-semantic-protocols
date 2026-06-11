//! Manifest-first dependency facts for ASP-owned search pipe graph requests.

use std::{collections::HashSet, fs, path::Path};

use toml::Value;

use super::search_pipe_model::Candidate;

const DEPENDENCY_FACT_LIMIT: usize = 48;

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
    }
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
    if language_id != "rust" {
        return;
    }
    let lock_versions = cargo_lock_versions(project_root);
    let manifest_dependencies = cargo_manifest_dependencies(project_root, &lock_versions);
    for (dependency, version) in manifest_dependencies {
        let key = format!("Cargo.toml:{dependency}:manifest");
        if seen_facts.insert(key) {
            facts.push(DependencyFact {
                owner_path: "Cargo.toml".to_string(),
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

fn normalize_dependency_name(name: &str) -> String {
    name.replace(['_', '.'], "-").to_ascii_lowercase()
}

fn dependency_from_line(language_id: &str, line: &str) -> Option<String> {
    match language_id {
        "rust" => rust_dependency_from_line(line),
        "typescript" => typescript_dependency_from_line(line),
        "python" => python_dependency_from_line(line),
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

fn quoted_dependency(line: &str) -> Option<String> {
    let start = line.find('"').or_else(|| line.find('\''))?;
    let quote = line.as_bytes()[start] as char;
    let rest = &line[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
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
