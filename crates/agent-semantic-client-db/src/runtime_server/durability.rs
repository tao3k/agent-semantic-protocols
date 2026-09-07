// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned durability attachment tasks and typed observations.

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceIndexDurabilityAttachmentReceipt<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    state: &'static str,
    project_id: &'a str,
    workspace_id: &'a str,
    generation_digest: &'a str,
    source_root_digest: &'a str,
    elapsed_micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

pub(super) fn emit_source_index_durability_attachment(
    state: &'static str,
    project_id: &str,
    workspace_id: &str,
    generation_digest: &str,
    source_root_digest: &str,
    elapsed_micros: u64,
    error: Option<&str>,
) {
    eprintln!(
        "[source-index-durability-attachment] {}",
        serde_json::to_string(&SourceIndexDurabilityAttachmentReceipt {
            schema_id: "agent.semantic-protocols.source-index-durability-attachment-receipt",
            schema_version: "1",
            state,
            project_id,
            workspace_id,
            generation_digest,
            source_root_digest,
            elapsed_micros,
            reason_kind: error.map(|_| "source-index-durability-attachment-failed"),
            error,
        })
        .unwrap_or_else(|encode_error| format!(
            "{{\"schemaId\":\"agent.semantic-protocols.source-index-durability-attachment-receipt\",\"schemaVersion\":\"1\",\"state\":\"failed\",\"reasonKind\":\"receipt-encoding-failed\",\"error\":{}}}",
            serde_json::Value::String(encode_error.to_string())
        ))
    );
}

pub(super) async fn spawn_runtime_owned_durability_task<F>(
    durability_tasks: &std::sync::Arc<tokio::sync::Mutex<tokio::task::JoinSet<()>>>,
    task: F,
) where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let mut durability_tasks = durability_tasks.lock().await;
    while durability_tasks.try_join_next().is_some() {}
    durability_tasks.spawn(task);
}

pub(super) fn durable_runtime_bundle_matches_current(
    observed_generation: Option<&str>,
    current_generation: Option<&str>,
) -> bool {
    matches!(
        (observed_generation, current_generation),
        (Some(observed), Some(current)) if !current.is_empty() && observed == current
    )
}
