use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use agent_semantic_client_db::ClientDbArtifactEvent;

use super::search_history_paths::{target_or_query, target_path};

const ARTIFACT_EVENT_DIRS: &[&str] = &[
    "prompt-output",
    "query",
    "search",
    "search-output",
    "semantic-tree-sitter-query",
];

pub(super) fn artifact_file_count(artifact_dir: &Path) -> Result<usize, String> {
    let mut count = 0_usize;
    for name in ARTIFACT_EVENT_DIRS {
        let dir = artifact_dir.join(name);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("failed to read artifact dir entry: {error}"))?;
            if entry
                .file_type()
                .map_err(|error| format!("failed to inspect artifact file type: {error}"))?
                .is_file()
            {
                count = count.saturating_add(1);
            }
        }
    }
    Ok(count)
}

pub(super) fn scan_artifact_events_for_db(
    artifact_dir: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    let workspace_root = artifact_workspace_root(artifact_dir);
    let mut events = Vec::new();
    for dir_name in ARTIFACT_EVENT_DIRS {
        events.extend(scan_artifact_dir_events(
            artifact_dir,
            dir_name,
            &workspace_root,
        )?);
    }
    events.sort_by(|left, right| {
        left.timestamp_ms
            .cmp(&right.timestamp_ms)
            .then_with(|| left.artifact_path.cmp(&right.artifact_path))
            .then_with(|| left.event_ordinal.cmp(&right.event_ordinal))
    });
    Ok(events)
}

fn scan_artifact_dir_events(
    artifact_dir: &Path,
    dir_name: &str,
    workspace_root: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    let dir = artifact_dir.join(dir_name);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut events = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read artifact dir entry: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("failed to inspect artifact file type: {error}"))?
            .is_file()
        {
            continue;
        }
        events.extend(artifact_path_events(
            artifact_dir,
            dir_name,
            &entry.path(),
            workspace_root,
        )?);
    }
    Ok(events)
}

fn artifact_path_events(
    artifact_dir: &Path,
    dir_name: &str,
    path: &Path,
    workspace_root: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    match dir_name {
        "prompt-output" => prompt_output_artifact_events(artifact_dir, path, workspace_root),
        "query" => packet_artifact_event(artifact_dir, path, "query", workspace_root),
        "search" => packet_artifact_event(artifact_dir, path, "search", workspace_root),
        "search-output" => text_artifact_event(artifact_dir, path, "search-output", workspace_root)
            .map(|event| vec![event]),
        "semantic-tree-sitter-query" => {
            packet_artifact_event(artifact_dir, path, "tree-sitter-query", workspace_root)
        }
        _ => Ok(Vec::new()),
    }
}

fn prompt_output_artifact_events(
    artifact_dir: &Path,
    path: &Path,
    workspace_root: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".command.json"))
    {
        return command_artifact_events(artifact_dir, path, workspace_root);
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("txt") {
        text_artifact_event(artifact_dir, path, "prompt-output", workspace_root)
            .map(|event| vec![event])
    } else {
        Ok(Vec::new())
    }
}

fn packet_artifact_event(
    artifact_dir: &Path,
    path: &Path,
    kind: &str,
    workspace_root: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
        return Ok(Vec::new());
    }
    let packet = load_json(path)?;
    let target = packet_target(&packet);
    let query = packet_query(&packet);
    let project_root = packet
        .get("projectRoot")
        .and_then(serde_json::Value::as_str)
        .filter(|root| !root.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            artifact_infer_project_root(workspace_root, target_or_query(&target, &query))
        });
    let language = packet
        .get("languageId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| language_from_name(path))
        .to_string();
    let method = packet
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| method_from_name(path));
    let method = if kind == "tree-sitter-query" && method == "query" {
        "query/tree-sitter".to_string()
    } else {
        method
    };
    Ok(vec![artifact_event(
        artifact_dir,
        path,
        ArtifactEventFields {
            event_ordinal: 0,
            kind,
            language: &language,
            method: &method,
            target: &target,
            query: &query,
            project_root: &project_root,
        },
        workspace_root,
    )?])
}

fn command_artifact_events(
    artifact_dir: &Path,
    path: &Path,
    workspace_root: &Path,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    let packet = load_json(path)?;
    let Some(commands) = packet
        .get("providerCommands")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(Vec::new());
    };
    let mut events = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let argv = command_argv(command.get("argv"));
        let target = command_target(&argv);
        let query = command_query(&argv);
        let project_root = command
            .get("projectRoot")
            .and_then(serde_json::Value::as_str)
            .filter(|root| !root.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                artifact_infer_project_root(workspace_root, target_or_query(&target, &query))
            });
        events.push(artifact_event(
            artifact_dir,
            path,
            ArtifactEventFields {
                event_ordinal: index.min(u32::MAX as usize) as u32,
                kind: "command",
                language: command
                    .get("languageId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_else(|| language_from_name(path)),
                method: &command_method(&argv),
                target: &target,
                query: &query,
                project_root: &project_root,
            },
            workspace_root,
        )?);
    }
    Ok(events)
}

fn text_artifact_event(
    artifact_dir: &Path,
    path: &Path,
    kind: &str,
    workspace_root: &Path,
) -> Result<ClientDbArtifactEvent, String> {
    artifact_event(
        artifact_dir,
        path,
        ArtifactEventFields {
            event_ordinal: 0,
            kind,
            language: language_from_name(path),
            method: &method_from_name(path),
            target: "",
            query: "",
            project_root: "",
        },
        workspace_root,
    )
}

struct ArtifactEventFields<'a> {
    event_ordinal: u32,
    kind: &'a str,
    language: &'a str,
    method: &'a str,
    target: &'a str,
    query: &'a str,
    project_root: &'a str,
}

fn artifact_event(
    artifact_dir: &Path,
    path: &Path,
    fields: ArtifactEventFields<'_>,
    workspace_root: &Path,
) -> Result<ClientDbArtifactEvent, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect artifact {}: {error}", path.display()))?;
    Ok(ClientDbArtifactEvent {
        artifact_path: artifact_relative_path(artifact_dir, path),
        event_ordinal: fields.event_ordinal,
        timestamp_ms: metadata_modified_ms(&metadata),
        kind: fields.kind.to_string(),
        language: fields.language.to_string(),
        method: fields.method.to_string(),
        target: fields.target.to_string(),
        query: fields.query.to_string(),
        project_root: fields.project_root.to_string(),
        project_root_arg: artifact_project_root_arg(fields.project_root, workspace_root),
        bytes: metadata.len(),
    })
}

fn metadata_modified_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn artifact_relative_path(artifact_dir: &Path, path: &Path) -> String {
    path.strip_prefix(artifact_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn load_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read artifact json {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse artifact json {}: {error}", path.display()))
}

fn packet_target(packet: &serde_json::Value) -> String {
    if let Some(owner) = packet.get("ownerPath").and_then(serde_json::Value::as_str) {
        return owner.to_string();
    }
    packet
        .get("owners")
        .and_then(serde_json::Value::as_array)
        .and_then(|owners| owners.first())
        .and_then(|owner| owner.get("path"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn packet_query(packet: &serde_json::Value) -> String {
    match packet.get("query") {
        Some(serde_json::Value::String(query)) => query.to_string(),
        Some(serde_json::Value::Object(query)) => query
            .get("input")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn command_argv(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|argv| {
            argv.iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn command_method(argv: &[String]) -> String {
    if let Some(search_index) = argv.iter().position(|arg| arg == "search") {
        let (surface, _) = command_surface(argv, search_index + 1);
        return format!("search/{}", surface.unwrap_or("unknown"));
    }
    if argv.iter().any(|arg| arg == "query") {
        if command_is_direct_source_read(argv) {
            return "query/direct-source-read".to_string();
        }
        if command_is_tree_sitter_query(argv) {
            return "query/tree-sitter".to_string();
        }
        if command_is_selector_code_query(argv) {
            return "query/code".to_string();
        }
        return "query".to_string();
    }
    "command/unknown".to_string()
}

fn command_target(argv: &[String]) -> String {
    if let Some(search_index) = argv.iter().position(|arg| arg == "search") {
        let (_, surface_index) = command_surface(argv, search_index + 1);
        return next_positional(argv, surface_index + 1)
            .unwrap_or("")
            .to_string();
    }
    if let Some(query_index) = argv.iter().position(|arg| arg == "query") {
        if let Some(selector) = option_value(argv, "--selector") {
            return selector.to_string();
        }
        return next_positional(argv, query_index + 1)
            .unwrap_or("")
            .to_string();
    }
    String::new()
}

fn command_query(argv: &[String]) -> String {
    if command_is_direct_source_read(argv) {
        return option_value(argv, "--term").unwrap_or("").to_string();
    }
    if command_is_tree_sitter_query(argv) {
        return tree_sitter_query_input(argv).unwrap_or("").to_string();
    }
    let Some(search_index) = argv.iter().position(|arg| arg == "search") else {
        return String::new();
    };
    let (surface, surface_index) = command_surface(argv, search_index + 1);
    if surface == Some("fzf") {
        next_positional(argv, surface_index + 1)
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    }
}

fn command_is_direct_source_read(argv: &[String]) -> bool {
    argv.iter().any(|arg| arg == "--from-hook")
        && argv.iter().any(|arg| arg == "direct-source-read")
}

fn command_is_selector_code_query(argv: &[String]) -> bool {
    option_value(argv, "--selector").is_some() && argv.iter().any(|arg| arg == "--code")
}

fn command_is_tree_sitter_query(argv: &[String]) -> bool {
    tree_sitter_query_input(argv).is_some()
}

fn tree_sitter_query_input(argv: &[String]) -> Option<&str> {
    [
        "--treesitter-query",
        "--tree-sitter-query",
        "--query-catalog",
    ]
    .iter()
    .find_map(|option| option_value(argv, option))
}

fn option_value<'a>(argv: &'a [String], option: &str) -> Option<&'a str> {
    argv.windows(2).find_map(|window| {
        if window[0] == option {
            Some(window[1].as_str())
        } else {
            None
        }
    })
}

fn command_surface(argv: &[String], start: usize) -> (Option<&str>, usize) {
    let mut index = start;
    while index < argv.len() {
        let item = argv[index].as_str();
        if item == "--" {
            index += 1;
        } else if item.starts_with('-') {
            index += if command_option_has_value(item) { 2 } else { 1 };
        } else {
            return (Some(item), index);
        }
    }
    (None, argv.len())
}

fn next_positional(argv: &[String], start: usize) -> Option<&str> {
    argv.iter()
        .skip(start)
        .find(|item| !item.starts_with('-'))
        .map(String::as_str)
}

fn command_option_has_value(option: &str) -> bool {
    matches!(
        option,
        "--dependency"
            | "--from-hook"
            | "--format"
            | "--owner"
            | "--package"
            | "--query"
            | "--query-set"
            | "--seeds"
            | "--selector"
            | "--view"
    )
}

fn artifact_workspace_root(root: &Path) -> PathBuf {
    let resolved = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for candidate in resolved.ancestors() {
        if candidate.file_name().and_then(|name| name.to_str()) == Some(".cache")
            && let Some(parent) = candidate.parent()
        {
            return parent.to_path_buf();
        }
    }
    if resolved.is_dir() {
        resolved
    } else {
        resolved.parent().unwrap_or(&resolved).to_path_buf()
    }
}

fn artifact_infer_project_root(workspace_root: &Path, target: &str) -> String {
    let Some(target_path) = target_path(target) else {
        return String::new();
    };
    if target_path.is_absolute() {
        return String::new();
    }
    let mut matches = candidate_project_roots(workspace_root)
        .into_iter()
        .filter(|candidate| candidate.join(&target_path).exists())
        .filter_map(|candidate| candidate.canonicalize().ok())
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    if matches.len() == 1 {
        matches[0].display().to_string()
    } else {
        String::new()
    }
}

fn candidate_project_roots(workspace_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![workspace_root.to_path_buf()];
    for base in [
        workspace_root.join("languages"),
        workspace_root.join("packages/python"),
    ] {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        roots.extend(entries.filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.path())
            } else {
                None
            }
        }));
    }
    roots
}

fn artifact_project_root_arg(project_root: &str, workspace_root: &Path) -> String {
    if project_root.is_empty() {
        return String::new();
    }
    let root_path = Path::new(project_root)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(project_root));
    let workspace = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    match root_path.strip_prefix(&workspace) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => root_path.display().to_string(),
    }
}

fn language_from_name(path: &Path) -> &str {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return "unknown";
    };
    name.split_once('-')
        .map(|(language, _)| language)
        .unwrap_or("unknown")
}

fn method_from_name(path: &Path) -> String {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return "unknown".to_string();
    };
    let stem = stem.strip_suffix(".command").unwrap_or(stem);
    let parts = stem.split('-').collect::<Vec<_>>();
    if parts.len() < 3 {
        "unknown".to_string()
    } else {
        parts[1..parts.len() - 1].join("/")
    }
}
