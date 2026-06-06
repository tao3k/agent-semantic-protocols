//! Search command history audit via the graph-turbo artifact timeline.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use agent_semantic_client_core::ClientCacheManifest;
use agent_semantic_client_db::{ClientDb, ClientDbArtifactEvent};

pub(crate) fn run_search_history(project_root: &Path, args: &[String]) -> Result<(), String> {
    let (audit_root, forwarded_args) = parse_history_audit_args(project_root, args)?;
    print_history_audit(&audit_root, forwarded_args)
}

fn parse_history_audit_args<'a>(
    project_root: &Path,
    args: &'a [String],
) -> Result<(PathBuf, &'a [String]), String> {
    let [subcommand, action, tail @ ..] = args else {
        return Err(usage());
    };
    if subcommand != "history" || action != "audit" {
        return Err(usage());
    }
    if tail.first().is_some_and(|arg| arg.starts_with('-')) {
        return Ok((project_root.to_path_buf(), tail));
    }
    match tail.split_first() {
        Some((root, forwarded)) => Ok((project_root.join(root), forwarded)),
        None => Ok((project_root.to_path_buf(), tail)),
    }
}

fn print_history_audit(audit_root: &Path, forwarded_args: &[String]) -> Result<(), String> {
    let artifact_dir = artifact_dir(audit_root);
    let events_packet = artifact_events_packet(audit_root, &artifact_dir)?;
    let output = run_graph_turbo_timeline(&artifact_dir, forwarded_args, events_packet)?;
    if !output.status.success() {
        return Err(format!(
            "graph-turbo timeline failed status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

fn run_graph_turbo_timeline(
    artifact_dir: &Path,
    forwarded_args: &[String],
    events_packet: Option<Vec<u8>>,
) -> Result<std::process::Output, String> {
    let mut command = graph_turbo_command(artifact_dir, forwarded_args, events_packet.is_some());
    let mut child = command
        .stdin(if events_packet.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!("failed to run graph-turbo timeline: {error}; install graph-turbo on PATH")
        })?;
    if let Some(packet) = events_packet {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open graph-turbo timeline stdin".to_string())?;
        stdin
            .write_all(&packet)
            .map_err(|error| format!("failed to write graph-turbo events json: {error}"))?;
    }
    child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for graph-turbo timeline: {error}"))
}

fn graph_turbo_command(
    artifact_dir: &Path,
    forwarded_args: &[String],
    use_events_json: bool,
) -> Command {
    let mut command = Command::new("graph-turbo");
    command.arg("timeline");
    command.arg(artifact_dir);
    if use_events_json {
        command.args(["--events-json", "-"]);
    }
    command.args(forwarded_args);
    command
}

fn artifact_dir(root: &Path) -> PathBuf {
    root.join(".cache/agent-semantic-protocol/artifacts")
}

fn artifact_events_packet(
    audit_root: &Path,
    artifact_dir: &Path,
) -> Result<Option<Vec<u8>>, String> {
    let cache_report = ClientCacheManifest::inspect_project(audit_root);
    let Some(cache_root) = cache_report
        .cache_root
        .or_else(|| artifact_dir.parent().map(Path::to_path_buf))
    else {
        return Ok(None);
    };
    let db_path = ClientDb::default_path(cache_root);
    let artifact_file_count = artifact_file_count(artifact_dir)?;
    let mut events = ClientDb::lookup_artifact_events(&db_path, None, 1_000_000)?;
    let indexed_artifact_count = events
        .iter()
        .map(|event| event.artifact_path.as_str())
        .collect::<HashSet<_>>()
        .len();
    if indexed_artifact_count < artifact_file_count {
        let backfill_events = scan_artifact_events_for_db(artifact_dir)?;
        if !backfill_events.is_empty() {
            let mut db = ClientDb::open_or_create(&db_path)?;
            db.upsert_artifact_events(&backfill_events)?;
            events = ClientDb::lookup_artifact_events(&db_path, None, 1_000_000)?;
        }
    }
    if events.is_empty() {
        return Ok(None);
    }
    let indexed_artifact_count = events
        .iter()
        .map(|event| event.artifact_path.as_str())
        .collect::<HashSet<_>>()
        .len();
    if indexed_artifact_count < artifact_file_count {
        return Ok(None);
    }
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-artifact-events",
        "schemaVersion": "1",
        "artifactDir": artifact_dir.display().to_string(),
        "source": {
            "kind": "rust-sqlite",
            "dbPath": db_path.display().to_string()
        },
        "events": events.iter().map(event_json).collect::<Vec<_>>()
    });
    serde_json::to_vec(&packet)
        .map(Some)
        .map_err(|error| format!("failed to encode graph-turbo events json: {error}"))
}

fn artifact_file_count(artifact_dir: &Path) -> Result<usize, String> {
    let mut count = 0_usize;
    for name in [
        "prompt-output",
        "query",
        "search",
        "search-output",
        "semantic-tree-sitter-query",
    ] {
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

fn scan_artifact_events_for_db(artifact_dir: &Path) -> Result<Vec<ClientDbArtifactEvent>, String> {
    let workspace_root = artifact_workspace_root(artifact_dir);
    let mut events = Vec::new();
    for name in [
        "prompt-output",
        "query",
        "search",
        "search-output",
        "semantic-tree-sitter-query",
    ] {
        let dir = artifact_dir.join(name);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("failed to read artifact dir entry: {error}"))?;
            if !entry
                .file_type()
                .map_err(|error| format!("failed to inspect artifact file type: {error}"))?
                .is_file()
            {
                continue;
            }
            let path = entry.path();
            match name {
                "prompt-output" => {
                    if path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".command.json"))
                    {
                        events.extend(command_artifact_events(
                            artifact_dir,
                            &path,
                            &workspace_root,
                        )?);
                    } else if path.extension().and_then(|ext| ext.to_str()) == Some("txt") {
                        events.push(text_artifact_event(
                            artifact_dir,
                            &path,
                            "prompt-output",
                            &workspace_root,
                        )?);
                    }
                }
                "query" => events.extend(packet_artifact_event(
                    artifact_dir,
                    &path,
                    "query",
                    &workspace_root,
                )?),
                "search" => events.extend(packet_artifact_event(
                    artifact_dir,
                    &path,
                    "search",
                    &workspace_root,
                )?),
                "search-output" => events.push(text_artifact_event(
                    artifact_dir,
                    &path,
                    "search-output",
                    &workspace_root,
                )?),
                "semantic-tree-sitter-query" => events.extend(packet_artifact_event(
                    artifact_dir,
                    &path,
                    "tree-sitter-query",
                    &workspace_root,
                )?),
                _ => {}
            }
        }
    }
    events.sort_by(|left, right| {
        left.timestamp_ms
            .cmp(&right.timestamp_ms)
            .then_with(|| left.artifact_path.cmp(&right.artifact_path))
            .then_with(|| left.event_ordinal.cmp(&right.event_ordinal))
    });
    Ok(events)
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
    let query = packet
        .get("query")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
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
    Ok(vec![artifact_event(
        artifact_dir,
        path,
        0,
        kind,
        &language,
        &method,
        &target,
        &query,
        &project_root,
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
            index.min(u32::MAX as usize) as u32,
            "command",
            command
                .get("languageId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| language_from_name(path)),
            &command_method(&argv),
            &target,
            &query,
            &project_root,
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
        0,
        kind,
        language_from_name(path),
        &method_from_name(path),
        "",
        "",
        "",
        workspace_root,
    )
}

fn artifact_event(
    artifact_dir: &Path,
    path: &Path,
    event_ordinal: u32,
    kind: &str,
    language: &str,
    method: &str,
    target: &str,
    query: &str,
    project_root: &str,
    workspace_root: &Path,
) -> Result<ClientDbArtifactEvent, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect artifact {}: {error}", path.display()))?;
    Ok(ClientDbArtifactEvent {
        artifact_path: artifact_relative_path(artifact_dir, path),
        event_ordinal,
        timestamp_ms: metadata_modified_ms(&metadata),
        kind: kind.to_string(),
        language: language.to_string(),
        method: method.to_string(),
        target: target.to_string(),
        query: query.to_string(),
        project_root: project_root.to_string(),
        project_root_arg: artifact_project_root_arg(project_root, workspace_root),
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
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read artifact json {}: {error}", path.display()))?;
    serde_json::from_str(&text)
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
        return next_positional(argv, query_index + 1)
            .unwrap_or("")
            .to_string();
    }
    String::new()
}

fn command_query(argv: &[String]) -> String {
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
        if candidate.file_name().and_then(|name| name.to_str()) == Some(".cache") {
            if let Some(parent) = candidate.parent() {
                return parent.to_path_buf();
            }
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

fn target_or_query<'a>(target: &'a str, query: &'a str) -> &'a str {
    if target.is_empty() { query } else { target }
}

fn target_path(value: &str) -> Option<PathBuf> {
    let value = strip_locator(value);
    if value.is_empty() || value.contains(' ') {
        return None;
    }
    let path = PathBuf::from(&value);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !value.contains('/') && !file_name.contains('.') {
        return None;
    }
    Some(path)
}

fn strip_locator(value: &str) -> String {
    let Some((head, tail)) = value.rsplit_once(':') else {
        return value.to_string();
    };
    if tail.chars().all(|ch| ch.is_ascii_digit()) || tail.contains('-') {
        head.to_string()
    } else {
        value.to_string()
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

fn event_json(event: &ClientDbArtifactEvent) -> serde_json::Value {
    serde_json::json!({
        "timestamp": event.timestamp_ms as f64 / 1000.0,
        "kind": event.kind,
        "language": event.language,
        "method": event.method,
        "target": event.target,
        "query": event.query,
        "projectRoot": event.project_root,
        "projectRootArg": event.project_root_arg,
        "path": event.artifact_path,
        "bytes": event.bytes
    })
}

fn usage() -> String {
    "usage: asp search history audit [PROJECT_ROOT] [GRAPH_TURBO_TIMELINE_ARGS...]".to_string()
}
