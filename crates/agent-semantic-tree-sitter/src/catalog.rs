//! Catalog loading and fingerprinting for tree-sitter-compatible `.scm` query surfaces.
//!
//! This module does not link tree-sitter runtime or grammar crates. It prepares
//! stable ASP metadata that later runtime, cache, or native-projection layers can consume.

use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::compile_query_abi_source;

/// Registry-declared tree-sitter-compatible query catalog entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCatalogDescriptor {
    pub id: String,
    pub path: PathBuf,
    pub declared_captures: Vec<String>,
}

/// Loaded canonical `.scm` query catalog content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedSyntaxCatalog {
    pub id: String,
    pub path: PathBuf,
    pub source: String,
    pub declared_captures: Vec<String>,
    pub discovered_captures: Vec<String>,
    pub fingerprint: String,
}

/// Loaded `grammar-profile.json` content for a provider grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedGrammarProfile {
    pub path: PathBuf,
    pub source: String,
    pub fingerprint: String,
}

/// Load a registry-declared catalog from a provider project root.
pub fn load_syntax_catalog(
    project_root: &Path,
    descriptor: &SyntaxCatalogDescriptor,
) -> Result<LoadedSyntaxCatalog, String> {
    let source = fs::read_to_string(project_root.join(&descriptor.path)).map_err(|error| {
        format!(
            "failed to read syntax query catalog {}: {error}",
            descriptor.path.display()
        )
    })?;
    let discovered_captures = compile_query_abi_source(&source)
        .map_err(|error| {
            format!(
                "failed to compile syntax query catalog {}: {}",
                descriptor.path.display(),
                error.message
            )
        })?
        .captures;
    let fingerprint = fingerprint_catalog(descriptor, &source);
    Ok(LoadedSyntaxCatalog {
        id: descriptor.id.clone(),
        path: descriptor.path.clone(),
        source,
        declared_captures: normalize_capture_names(&descriptor.declared_captures),
        discovered_captures,
        fingerprint,
    })
}

/// Load a provider grammar profile from a provider project root.
pub fn load_grammar_profile(
    project_root: &Path,
    profile_path: impl Into<PathBuf>,
) -> Result<LoadedGrammarProfile, String> {
    let path = profile_path.into();
    let source = fs::read_to_string(project_root.join(&path)).map_err(|error| {
        format!(
            "failed to read syntax grammar profile {}: {error}",
            path.display()
        )
    })?;
    let fingerprint = fingerprint_grammar_profile(&path, &source);
    Ok(LoadedGrammarProfile {
        path,
        source,
        fingerprint,
    })
}

/// Extract capture names from tree-sitter query source without compiling a grammar.
#[must_use]
pub fn extract_capture_names(source: &str) -> Vec<String> {
    compile_query_abi_source(source)
        .map(|plan| plan.captures)
        .unwrap_or_default()
}

/// Normalize capture names into stable ABI order.
#[must_use]
pub fn normalize_capture_names(captures: &[String]) -> Vec<String> {
    captures
        .iter()
        .filter(|capture| !capture.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Stable local fingerprint for catalog metadata and source.
#[must_use]
pub fn fingerprint_catalog(descriptor: &SyntaxCatalogDescriptor, source: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    descriptor.id.hash(&mut hasher);
    descriptor.path.hash(&mut hasher);
    source.hash(&mut hasher);
    format!("syntax-catalog:{:016x}", hasher.finish())
}

/// Stable local fingerprint for grammar profile metadata and source.
#[must_use]
pub fn fingerprint_grammar_profile(path: &Path, source: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    source.hash(&mut hasher);
    format!("grammar-profile:{:016x}", hasher.finish())
}
