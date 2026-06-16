//! Tokenization and language inference helpers for source-index rows.

use std::collections::BTreeSet;
use std::path::Path;

use super::config::SOURCE_INDEX_QUERY_KEY_LIMIT;

pub(super) fn source_line_count(text: &str) -> u32 {
    text.lines().count().max(1).min(u32::MAX as usize) as u32
}

pub(super) fn source_query_keys(path: &str, text: &str) -> Vec<String> {
    let mut keys = BTreeSet::new();
    append_source_tokens(path, &mut keys);
    append_source_tokens(text, &mut keys);
    keys.into_iter()
        .take(SOURCE_INDEX_QUERY_KEY_LIMIT)
        .collect()
}

fn append_source_tokens(text: &str, keys: &mut BTreeSet<String>) {
    let mut token = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '/') {
            token.push(character.to_ascii_lowercase());
        } else {
            push_source_token(&mut token, keys);
        }
    }
    push_source_token(&mut token, keys);
}

fn push_source_token(token: &mut String, keys: &mut BTreeSet<String>) {
    let value = token.trim_matches([':', '/', '-', '_']);
    if value.len() >= 2 {
        keys.insert(value.to_string());
    }
    token.clear();
}

pub(super) fn source_language_id(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => Some("rust"),
        Some("ts" | "tsx" | "js" | "jsx") => Some("typescript"),
        Some("py") => Some("python"),
        Some("jl") => Some("julia"),
        Some("ss" | "ssi" | "scm" | "sld") => Some("gerbil-scheme"),
        Some("org") => Some("org"),
        Some("md") => Some("md"),
        _ => None,
    }
}

pub(super) fn lookup_terms(query: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        terms.insert(trimmed.to_ascii_lowercase());
    }
    for term in query
        .split(|character: char| {
            !(character == '_'
                || character == '-'
                || character == ':'
                || character == '/'
                || character.is_ascii_alphanumeric())
        })
        .map(str::trim)
        .filter(|term| !term.is_empty())
    {
        terms.insert(term.to_ascii_lowercase());
    }
    terms.into_iter().collect()
}
