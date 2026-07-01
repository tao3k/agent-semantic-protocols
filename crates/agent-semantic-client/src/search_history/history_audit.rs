//! Search command history audit via the graph-turbo artifact timeline.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use agent_semantic_client_core::ProjectContext;
use agent_semantic_client_db::{ClientDbArtifactEvent, ClientDbEngine};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessOutput, ProviderProcessSpec, StdinMode,
    run_provider_process,
};
use bytes::Bytes;

use super::artifact_events::{artifact_file_count, scan_artifact_events_for_db};

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
    let project_context = ProjectContext::resolve(audit_root)?;
    let artifact_dir = project_context.state_layout().artifacts_dir().to_path_buf();
    let events_packet = artifact_events_packet(&project_context, &artifact_dir)?;
    let output = run_graph_turbo_timeline(&artifact_dir, forwarded_args, events_packet)?;
    if !output.status.success() {
        return Err(format!(
            "graph-turbo timeline failed status={} stderr={}",
            output.status,
            output.stderr_lossy().trim()
        ));
    }
    print!("{}", output.stdout_lossy());
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr_lossy());
    }
    Ok(())
}

fn run_graph_turbo_timeline(
    artifact_dir: &Path,
    forwarded_args: &[String],
    events_packet: Option<Bytes>,
) -> Result<ProviderProcessOutput, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let mut args = vec!["timeline".to_string(), artifact_dir.display().to_string()];
    let stdin = if let Some(packet) = events_packet {
        args.extend(["--events-json".to_string(), "-".to_string()]);
        StdinMode::bytes(packet)
    } else {
        StdinMode::Closed
    };
    args.extend(forwarded_args.iter().cloned());
    run_provider_process(ProviderProcessSpec {
        program: "asp-graph-turbo".to_string(),
        args,
        cwd,
        env: BTreeMap::new(),
        stdin,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    })
    .map_err(|error| {
        format!(
            "failed to run asp-graph-turbo timeline: {error}; run just agent-tools-install-asp-graph-turbo <bin-dir>"
        )
    })
}

fn artifact_events_packet(
    project_context: &ProjectContext,
    artifact_dir: &Path,
) -> Result<Option<Bytes>, String> {
    let artifact_file_count = artifact_file_count(artifact_dir)?;
    let mut events = ClientDbEngine::lookup_artifact_events_from_client_dir(
        project_context.state_layout().client_cache_dir(),
        None,
        1_000_000,
    )?;
    let indexed_count = indexed_artifact_count(&events);
    if indexed_count < artifact_file_count {
        let backfill_events = scan_artifact_events_for_db(artifact_dir)?;
        if !backfill_events.is_empty() {
            ClientDbEngine::upsert_artifact_events_from_client_dir(
                project_context.state_layout().client_cache_dir(),
                &backfill_events,
            )?;
            events = ClientDbEngine::lookup_artifact_events_from_client_dir(
                project_context.state_layout().client_cache_dir(),
                None,
                1_000_000,
            )?;
        }
    }
    if events.is_empty() || indexed_artifact_count(&events) < artifact_file_count {
        return Ok(None);
    }
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-artifact-events",
        "schemaVersion": "1",
        "artifactDir": artifact_dir.display().to_string(),
        "source": {
            "kind": "db-engine",
            "clientDir": project_context.state_layout().client_cache_dir().display().to_string()
        },
        "events": events.iter().map(|event| {
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
        }).collect::<Vec<_>>()
    });
    serde_json::to_vec(&packet)
        .map(Bytes::from)
        .map(Some)
        .map_err(|error| format!("failed to encode graph-turbo events json: {error}"))
}

fn indexed_artifact_count(events: &[ClientDbArtifactEvent]) -> usize {
    events
        .iter()
        .map(|event| event.artifact_path.as_str())
        .collect::<HashSet<_>>()
        .len()
}

fn usage() -> String {
    "usage: asp search history audit [PROJECT_ROOT] [GRAPH_TURBO_TIMELINE_ARGS...]".to_string()
}
