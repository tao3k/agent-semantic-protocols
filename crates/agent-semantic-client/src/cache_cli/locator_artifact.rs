//! Locator-derived cache artifact helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, ClientCacheFileHash, ClientCacheGeneration, ClientRequest, ResolvedProvider,
};
use sha2::{Digest, Sha256};

use crate::cache_replay::{
    MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path, search_output_artifact_replay_safe,
};

pub(super) fn prompt_output_file_hashes(
    project_root: &Path,
    stdout: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    let text = std::str::from_utf8(stdout).ok()?;
    let mut paths = BTreeSet::new();
    for line in text.lines() {
        collect_locator_paths(line, &mut paths);
    }
    let file_hashes = paths
        .into_iter()
        .filter_map(|path| hash_project_file(project_root, &path))
        .collect::<Vec<_>>();
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

pub(super) fn query_selector_file_hashes(
    project_root: &Path,
    forwarded_args: &[String],
) -> Option<Vec<ClientCacheFileHash>> {
    let selector = option_value(forwarded_args, "--selector")?;
    let path = selector_path(selector)?;
    hash_project_file(project_root, path).map(|file_hash| vec![file_hash])
}

pub(super) fn search_output_file_hashes(
    project_root: &Path,
    package_roots: &[String],
    stdout: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    if !search_output_artifact_replay_safe(stdout) {
        return None;
    }
    locator_file_hashes_from_text(
        project_root,
        package_roots,
        std::str::from_utf8(stdout).ok()?,
    )
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == option {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg
            .strip_prefix(option)
            .and_then(|arg| arg.strip_prefix('='))
        {
            return Some(value);
        }
    }
    None
}

fn selector_path(selector: &str) -> Option<&str> {
    let normalized = selector.strip_prefix("owner:").unwrap_or(selector);
    let (path_or_path_and_start, range_or_end_text) = normalized.rsplit_once(':')?;
    if path_or_path_and_start.is_empty() || range_or_end_text.is_empty() {
        return None;
    }
    if is_line_range(range_or_end_text) {
        return Some(path_or_path_and_start);
    }
    let (path, start_text) = path_or_path_and_start.rsplit_once(':')?;
    if path.is_empty()
        || start_text.is_empty()
        || !is_line_number(start_text)
        || !is_line_number(range_or_end_text)
    {
        return None;
    }
    Some(path)
}

fn is_line_range(value: &str) -> bool {
    value
        .split_once('-')
        .is_some_and(|(start, end)| is_line_number(start) && is_line_number(end))
        || is_line_number(value)
}

fn is_line_number(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

pub(super) fn search_packet_file_hashes_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    packet_bytes: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    packet_file_hashes_from_packet(packet_bytes)
        .or_else(|| {
            locator_file_hashes_from_packet(project_root, &provider.package_roots, packet_bytes)
        })
        .or_else(|| {
            if request
                .forwarded_args
                .first()
                .is_none_or(|arg| arg != "prime")
            {
                return None;
            }
            let file_hashes = [
                ".cache/agent-semantic-protocol/hooks/activation.json",
                "Cargo.toml",
                "package.json",
                "tsconfig.json",
                "pyproject.toml",
                "Project.toml",
            ]
            .into_iter()
            .filter_map(|path| hash_project_file(project_root, path))
            .collect::<Vec<_>>();
            if file_hashes.is_empty() {
                None
            } else {
                Some(file_hashes)
            }
        })
}

pub(super) fn locator_file_hashes_from_packet(
    project_root: &Path,
    package_roots: &[String],
    packet_bytes: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let mut paths = BTreeSet::new();
    collect_json_locator_paths(&packet, &mut paths, None);
    locator_file_hashes_from_paths(project_root, package_roots, paths)
}

pub(super) fn maybe_write_search_output_artifact(
    cache_root: &Path,
    generation: &mut ClientCacheGeneration,
    stdout: &[u8],
) {
    if stdout.is_empty()
        || stdout.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
        || !search_output_artifact_replay_safe(stdout)
    {
        return;
    }
    let artifact_id = CacheArtifactId::from(format!(
        "search-output/{}.txt",
        generation.generation_id.as_str()
    ));
    let Some(artifact_path) =
        replay_artifact_path(cache_root, &artifact_id, "search-output/", ".txt")
    else {
        return;
    };
    let Some(parent) = artifact_path.parent() else {
        return;
    };
    if fs::create_dir_all(parent)
        .and_then(|_| fs::write(&artifact_path, stdout))
        .is_ok()
    {
        generation
            .artifact_ids
            .get_or_insert_with(Vec::new)
            .push(artifact_id);
    }
}

fn packet_file_hashes_from_packet(packet_bytes: &[u8]) -> Option<Vec<ClientCacheFileHash>> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let hashes = packet.pointer("/cache/fileHashes")?.as_array()?;
    let mut file_hashes = Vec::with_capacity(hashes.len());
    for hash in hashes {
        file_hashes.push(ClientCacheFileHash {
            path: hash.get("path")?.as_str()?.to_string(),
            sha256: hash.get("sha256")?.as_str()?.to_string(),
        });
    }
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn locator_file_hashes_from_text(
    project_root: &Path,
    package_roots: &[String],
    text: &str,
) -> Option<Vec<ClientCacheFileHash>> {
    let mut paths = BTreeSet::new();
    text.lines()
        .for_each(|line| collect_locator_paths(line, &mut paths));
    locator_file_hashes_from_paths(project_root, package_roots, paths)
}

fn locator_file_hashes_from_paths(
    project_root: &Path,
    package_roots: &[String],
    paths: BTreeSet<String>,
) -> Option<Vec<ClientCacheFileHash>> {
    let file_hashes = paths
        .into_iter()
        .flat_map(|path| hash_locator_file(project_root, package_roots, &path))
        .fold(BTreeMap::new(), |mut file_hashes, file_hash| {
            file_hashes
                .entry(file_hash.path.clone())
                .or_insert(file_hash);
            file_hashes
        })
        .into_values()
        .collect::<Vec<_>>();
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn collect_json_locator_paths(
    value: &serde_json::Value,
    paths: &mut BTreeSet<String>,
    key: Option<&str>,
) {
    match value {
        serde_json::Value::String(text) if key.is_some_and(is_locator_key) => {
            collect_locator_paths(text, paths);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_locator_paths(item, paths, None);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                collect_json_locator_paths(value, paths, Some(key));
            }
        }
        _ => {}
    }
}

fn is_locator_key(key: &str) -> bool {
    matches!(
        key,
        "selector"
            | "read"
            | "exactRead"
            | "path"
            | "target"
            | "ownerPath"
            | "matchLocator"
            | "captureLocator"
    )
}

pub(super) fn collect_locator_paths(line: &str, paths: &mut BTreeSet<String>) {
    for token in line.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(character, ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}')
        });
        if token.contains(';') {
            for segment in token.split(';') {
                collect_locator_paths(segment, paths);
            }
            continue;
        }
        if collect_compact_graph_path_tokens(token, paths) {
            continue;
        }
        let token = token
            .strip_prefix("owner:")
            .or_else(|| token.strip_prefix("path:"))
            .or_else(|| token.strip_prefix("path="))
            .or_else(|| token.strip_prefix("read="))
            .or_else(|| token.strip_prefix("target="))
            .unwrap_or(token);
        let path = strip_locator_suffix(token);
        if looks_like_source_path(path) {
            paths.insert(path.to_string());
        }
    }
}

fn collect_compact_graph_path_tokens(token: &str, paths: &mut BTreeSet<String>) -> bool {
    let mut remaining = token;
    let mut found = false;
    while let Some(index) = remaining.find(":path(") {
        let start = index + ":path(".len();
        let Some(end) = remaining[start..].find(')') else {
            break;
        };
        let path = &remaining[start..start + end];
        if looks_like_source_path(path) {
            paths.insert(path.to_string());
            found = true;
        }
        remaining = &remaining[start + end + 1..];
    }
    found
}

fn strip_locator_suffix(value: &str) -> &str {
    let Some((index, _)) = value
        .char_indices()
        .find(|(_, character)| *character == ':')
    else {
        return value;
    };
    let suffix = &value[index + 1..];
    if !suffix.is_empty()
        && suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == ':')
    {
        &value[..index]
    } else {
        value
    }
}

fn looks_like_source_path(value: &str) -> bool {
    value.ends_with(".rs")
        || value.ends_with(".ts")
        || value.ends_with(".tsx")
        || value.ends_with(".js")
        || value.ends_with(".jsx")
        || value.ends_with(".py")
        || value.ends_with(".jl")
}

fn hash_project_file(project_root: &Path, path: &str) -> Option<ClientCacheFileHash> {
    let file_path = safe_project_file_path(project_root, path)?;
    let bytes = fs::read(file_path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(ClientCacheFileHash {
        path: path.to_string(),
        sha256: format!("{digest:x}"),
    })
}

fn hash_locator_file(
    project_root: &Path,
    package_roots: &[String],
    path: &str,
) -> Vec<ClientCacheFileHash> {
    std::iter::once(path.to_string())
        .chain(package_roots.iter().filter_map(|package_root| {
            if package_root == "." || package_root.is_empty() {
                return None;
            }
            Some(format!(
                "{}/{}",
                package_root.trim_end_matches('/'),
                path.trim_start_matches("./")
            ))
        }))
        .filter_map(|candidate_path| hash_project_file(project_root, &candidate_path))
        .collect()
}

fn safe_project_file_path(project_root: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(project_root.join(relative))
}
