//! Bounded dependency facts for ASP-owned search pipe graph requests.

use std::{collections::HashSet, fs};

use super::search_pipe_model::Candidate;

const DEPENDENCY_SCAN_LINE_LIMIT: usize = 256;
const DEPENDENCY_FACT_LIMIT: usize = 24;

#[derive(Debug, Clone)]
pub(super) struct DependencyFact {
    pub(super) owner_path: String,
    pub(super) dependency: String,
}

pub(super) fn collect_dependency_facts(
    language_id: &str,
    query: Option<&str>,
    candidates: &[Candidate],
) -> Vec<DependencyFact> {
    let mut seen_facts = HashSet::new();
    let mut scanned_paths = HashSet::new();
    let mut facts = Vec::new();
    for candidate in candidates.iter().take(12) {
        push_dependency_fact(
            language_id,
            &candidate.path,
            &candidate.text,
            &mut seen_facts,
            &mut facts,
        );
        if scanned_paths.insert(candidate.path.clone()) {
            push_file_dependency_facts(language_id, &candidate.path, &mut seen_facts, &mut facts);
        }
        if facts.len() >= DEPENDENCY_FACT_LIMIT {
            break;
        }
    }
    if let Some(query) = query {
        facts.sort_by_key(|fact| !dependency_matches_query(&fact.dependency, query));
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

fn push_file_dependency_facts(
    language_id: &str,
    owner_path: &str,
    seen_facts: &mut HashSet<String>,
    facts: &mut Vec<DependencyFact>,
) {
    let Ok(source) = fs::read_to_string(owner_path) else {
        return;
    };
    for line in source.lines().take(DEPENDENCY_SCAN_LINE_LIMIT) {
        push_dependency_fact(language_id, owner_path, line, seen_facts, facts);
        if facts.len() >= DEPENDENCY_FACT_LIMIT {
            break;
        }
    }
}

fn push_dependency_fact(
    language_id: &str,
    owner_path: &str,
    line: &str,
    seen_facts: &mut HashSet<String>,
    facts: &mut Vec<DependencyFact>,
) {
    let Some(dependency) = dependency_from_line(language_id, line) else {
        return;
    };
    let key = format!("{owner_path}:{dependency}");
    if seen_facts.insert(key) {
        facts.push(DependencyFact {
            owner_path: owner_path.to_string(),
            dependency,
        });
    }
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
