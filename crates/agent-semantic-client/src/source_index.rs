//! Rust-owned source index refresh and lookup facade.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, project_client_cache_dir,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSource,
};
use sha2::{Digest, Sha256};

const SOURCE_INDEX_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-source-index";
const SOURCE_INDEX_SCHEMA_VERSION: &str = "1";
const SOURCE_INDEX_PROVIDER_ID: &str = "rust-sql-source-index";
const SOURCE_INDEX_QUERY_KEY_LIMIT: usize = 128;
const SOURCE_INDEX_FILE_LIMIT: usize = 4096;
const SOURCE_INDEX_FILE_BYTES_LIMIT: u64 = 1_048_576;
const SOURCE_INDEX_SKIP_DIRS: &[&str] = &[
    ".git",
    ".cache",
    ".gerbil",
    ".jj",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
];
const SOURCE_INDEX_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "jl", "ss", "ssi", "scm", "sld", "org", "md",
];
const SOURCE_INDEX_CONFIG_FILENAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pnpm-workspace.yaml",
    "pyproject.toml",
    "Project.toml",
    "gerbil.pkg",
    "build.ss",
];

/// Result of refreshing the Rust SQL source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRefreshReport {
    pub db_path: PathBuf,
    pub generation_id: CacheGenerationId,
    pub file_count: u32,
    pub owner_count: u32,
    pub selector_count: u32,
}

/// Source-index lookup state for agent-facing search fallbacks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexLookupState {
    MissingDb,
    EmptyIndex,
    Hit,
    Miss,
}

impl SourceIndexLookupState {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingDb => "missing-db",
            Self::EmptyIndex => "empty-index",
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }
}

/// Agent-facing source-index candidate row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexCandidate {
    pub path: String,
    pub language_id: Option<LanguageId>,
    pub provider_id: Option<ProviderId>,
    pub source_kind: String,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
}

/// Lookup result from the Rust SQL source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexLookupResult {
    pub db_path: PathBuf,
    pub state: SourceIndexLookupState,
    pub candidates: Vec<SourceIndexCandidate>,
}

/// Refresh the Rust SQL source index for a project without storing raw source.
pub fn refresh_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let cache_root = project_client_cache_dir(project_root)?;
    let db_path = ClientDb::default_path(&cache_root);
    let files = collect_source_index_files(project_root)?;
    let generation_id = source_index_generation_id();
    let import = source_index_import(project_root, generation_id.clone(), &files)?;
    let mut db = ClientDb::open_or_create(&db_path)?;
    let stats = db.replace_source_index(&import)?;
    Ok(SourceIndexRefreshReport {
        db_path,
        generation_id,
        file_count: files.len().min(u32::MAX as usize) as u32,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    })
}

/// Lookup source-index owners from the Rust SQL cache.
pub fn lookup_source_index(
    project_root: &Path,
    query: &str,
    limit: u32,
) -> Result<SourceIndexLookupResult, String> {
    let cache_root = project_client_cache_dir(project_root)?;
    let db_path = ClientDb::default_path(&cache_root);
    let Some(db) = ClientDb::open_read_only_existing(&db_path)? else {
        return Ok(SourceIndexLookupResult {
            db_path,
            state: SourceIndexLookupState::MissingDb,
            candidates: Vec::new(),
        });
    };
    let summary = db.summary()?;
    if summary.source_index_owner_count == 0 {
        return Ok(SourceIndexLookupResult {
            db_path,
            state: SourceIndexLookupState::EmptyIndex,
            candidates: Vec::new(),
        });
    }
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for term in lookup_terms(query) {
        if candidates.len() >= limit as usize {
            break;
        }
        let remaining = limit.saturating_sub(candidates.len() as u32);
        let owners = db.lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: project_root.to_path_buf(),
            query: ClientDbSourceIndexQueryKey::from(term),
            limit: remaining,
        })?;
        for owner in owners {
            if candidates.len() >= limit as usize {
                break;
            }
            if seen.insert(owner.owner_path.as_str().to_string()) {
                candidates.push(source_index_candidate(owner));
            }
        }
    }
    let state = if candidates.is_empty() {
        SourceIndexLookupState::Miss
    } else {
        SourceIndexLookupState::Hit
    };
    Ok(SourceIndexLookupResult {
        db_path,
        state,
        candidates,
    })
}

fn collect_source_index_files(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_source_index_files_in(project_root, project_root, &mut files)?;
    files.sort();
    files.truncate(SOURCE_INDEX_FILE_LIMIT);
    Ok(files)
}

fn collect_source_index_files_in(
    project_root: &Path,
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if files.len() >= SOURCE_INDEX_FILE_LIMIT || should_skip_source_index_dir(project_root, dir) {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read source index dir {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read source index entry under {}: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect source index path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_source_index_files_in(project_root, &path, files)?;
        } else if file_type.is_file() && supported_source_index_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn should_skip_source_index_dir(project_root: &Path, dir: &Path) -> bool {
    if dir == project_root {
        return false;
    }
    dir.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SOURCE_INDEX_SKIP_DIRS.contains(&name))
}

fn supported_source_index_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SOURCE_INDEX_EXTENSIONS.contains(&extension))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| SOURCE_INDEX_CONFIG_FILENAMES.contains(&name))
}

fn source_index_import(
    project_root: &Path,
    generation_id: CacheGenerationId,
    files: &[PathBuf],
) -> Result<ClientDbSourceIndexImport, String> {
    let mut file_hashes = Vec::with_capacity(files.len());
    let mut owners = Vec::with_capacity(files.len());
    let mut selectors = Vec::with_capacity(files.len());
    for path in files {
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "failed to read source index file {}: {error}",
                path.display()
            )
        })?;
        let relative_path = relative_project_path(project_root, path);
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        file_hashes.push(ClientCacheFileHash {
            path: relative_path.clone(),
            sha256,
        });
        let text = if bytes.len() as u64 <= SOURCE_INDEX_FILE_BYTES_LIMIT {
            String::from_utf8(bytes).unwrap_or_default()
        } else {
            String::new()
        };
        let line_count = source_line_count(&text);
        let query_keys = source_query_keys(&relative_path, &text);
        let owner_path = ClientDbSourceIndexPath::from(relative_path.clone());
        owners.push(ClientDbSourceIndexOwner {
            owner_path: owner_path.clone(),
            language_id: source_language_id(path).map(LanguageId::from),
            provider_id: Some(ProviderId::from(SOURCE_INDEX_PROVIDER_ID)),
            source_kind: ClientDbSourceIndexSource::from("file"),
            line_count: Some(line_count),
            query_keys: query_keys
                .iter()
                .cloned()
                .map(ClientDbSourceIndexQueryKey::from)
                .collect(),
        });
        selectors.push(ClientDbSourceIndexSelector {
            owner_path,
            selector_id: format!("{relative_path}:1:{}", line_count.max(1)),
            symbol: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string),
            kind: Some("file".to_string()),
            start_line: 1,
            end_line: line_count.max(1),
            source: ClientDbSourceIndexSource::from(SOURCE_INDEX_PROVIDER_ID),
            query_keys: query_keys
                .into_iter()
                .map(ClientDbSourceIndexQueryKey::from)
                .collect(),
        });
    }
    Ok(ClientDbSourceIndexImport {
        generation_id,
        project_root: project_root.to_path_buf(),
        schema_id: SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: SOURCE_INDEX_SCHEMA_VERSION.into(),
        file_hashes,
        owners,
        selectors,
    })
}

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_line_count(text: &str) -> u32 {
    text.lines().count().max(1).min(u32::MAX as usize) as u32
}

fn source_query_keys(path: &str, text: &str) -> Vec<String> {
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

fn source_language_id(path: &Path) -> Option<&'static str> {
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

fn lookup_terms(query: &str) -> Vec<String> {
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

fn source_index_candidate(owner: ClientDbSourceIndexOwner) -> SourceIndexCandidate {
    SourceIndexCandidate {
        path: owner.owner_path.as_str().to_string(),
        language_id: owner.language_id,
        provider_id: owner.provider_id,
        source_kind: owner.source_kind.as_str().to_string(),
        line_count: owner.line_count,
        query_keys: owner
            .query_keys
            .into_iter()
            .map(|key| key.as_str().to_string())
            .collect(),
    }
}

fn source_index_generation_id() -> CacheGenerationId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    CacheGenerationId::from(format!("source-index-{nanos}"))
}
