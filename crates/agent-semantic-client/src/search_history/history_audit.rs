//! Search command history audit via the graph-turbo artifact timeline.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_client_core::ProjectContext;
use agent_semantic_client_db::ClientDbArtifactEvent;
use agent_semantic_client_db::ClientDbEngine;
use bytes::Bytes;

use super::artifact_events::artifact_file_count;
use super::artifact_events::scan_artifact_events_for_db;

pub(crate) async fn run_search_history(project_root: &Path, args: &[String]) -> Result<(), String> {
    let (audit_root, forwarded_args) = parse_history_audit_args(project_root, args)?;
    print_history_audit(&audit_root, forwarded_args).await
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

async fn print_history_audit(audit_root: &Path, forwarded_args: &[String]) -> Result<(), String> {
    let project_context = ProjectContext::resolve(audit_root)?;
    let artifact_dir = project_context.state_layout().artifacts_dir().to_path_buf();
    let events_packet =
        artifact_events_packet(&project_context, &artifact_dir)?.ok_or_else(|| {
            "graph timeline requires a complete schema-owned artifact-event packet".to_owned()
        })?;
    let event_packet: serde_json::Value = serde_json::from_slice(&events_packet)
        .map_err(|error| format!("decode graph-turbo events packet: {error}"))?;
    let client = crate::runtime_language_client::AspClient::new(
        agent_semantic_runtime::resolve_state_home()?,
        audit_root.to_path_buf(),
    );
    let report = client
        .graphs_timeline(serde_json::json!({
            "schemaId": agent_semantic_client_protocol::GRAPH_TIMELINE_REQUEST_SCHEMA_ID,
        "schemaVersion": agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION,
            "eventPacket": event_packet,
            "arguments": forwarded_args,
        }))
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("encode graph timeline report: {error}"))?
    );
    Ok(())
}

pub(crate) fn artifact_events_packet(
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
                "timestamp": event.timestamp_ms() as f64 / 1000.0,
                "kind": event.kind(),
                "language": event.language(),
                "method": event.method(),
                "target": event.target(),
                "query": event.query(),
                "projectRoot": event.project_root(),
                "projectRootArg": event.project_root_arg(),
                "path": event.artifact_path(),
                "bytes": event.bytes()
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
        .map(|event| event.artifact_path())
        .collect::<HashSet<_>>()
        .len()
}

fn usage() -> String {
    "usage: asp search history audit [PROJECT_ROOT] [GRAPH_TURBO_TIMELINE_ARGS...]".to_string()
}
