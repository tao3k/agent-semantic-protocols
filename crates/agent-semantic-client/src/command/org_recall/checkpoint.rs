use super::{
    memory,
    model::{OrgPlanCandidate, OrgTaskCandidate},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{env, path::Path};

pub(super) struct CheckpointSyncOptions<'a> {
    pub(super) project: &'a str,
    pub(super) session: &'a str,
    pub(super) branch: Option<&'a str>,
    pub(super) state: Option<&'a Path>,
    pub(super) embedding_dim: Option<&'a str>,
    pub(super) project_root: &'a Path,
}

pub(super) struct CheckpointSyncResult {
    pub(super) checkpoints: usize,
    pub(super) skipped_session_plans: usize,
    pub(super) transport: &'static str,
}

#[derive(Serialize)]
struct CheckpointPutPayload {
    #[serde(rename = "sessionId")]
    session_id: String,
    title: String,
    status: &'static str,
    kind: String,
    #[serde(rename = "projectId")]
    project_id: String,
    #[serde(rename = "planId")]
    plan_id: String,
    #[serde(rename = "branchId", skip_serializing_if = "Option::is_none")]
    branch_id: Option<String>,
    #[serde(rename = "sourceLocator")]
    source_locator: String,
    #[serde(rename = "resumeCommand")]
    resume_command: String,
    metadata: Value,
}

#[derive(Serialize)]
struct MemoryWorkerCheckpointPutRequest<'a> {
    command: &'static str,
    payload: &'a CheckpointPutPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(rename = "embeddingDim", skip_serializing_if = "Option::is_none")]
    embedding_dim: Option<&'a str>,
}

pub(super) fn sync_checkpoints_with_memory_engine(
    candidates: &[OrgPlanCandidate],
    options: CheckpointSyncOptions<'_>,
) -> Result<CheckpointSyncResult, String> {
    if options.session.trim().is_empty() {
        return Err(
            "asp org recall plans --checkpoint-sync requires --session or agent session env"
                .to_string(),
        );
    }
    let mut payloads = Vec::new();
    let mut skipped_session_plans = 0usize;
    for candidate in candidates {
        if !matches_session(candidate, options.session) {
            skipped_session_plans += 1;
            continue;
        }
        for task in &candidate.task_candidates {
            payloads.push(checkpoint_payload(candidate, task, &options));
        }
    }
    if payloads.is_empty() {
        return Ok(CheckpointSyncResult {
            checkpoints: 0,
            skipped_session_plans,
            transport: "none",
        });
    }

    let mut transport = "none";
    for payload in &payloads {
        transport = put_checkpoint(payload, &options)?;
    }
    Ok(CheckpointSyncResult {
        checkpoints: payloads.len(),
        skipped_session_plans,
        transport,
    })
}

fn put_checkpoint(
    payload: &CheckpointPutPayload,
    options: &CheckpointSyncOptions<'_>,
) -> Result<&'static str, String> {
    let request_body = serde_json::to_vec(payload)
        .map_err(|error| format!("failed to encode memory checkpoint request: {error}"))?;
    let mut checkpoint_args = vec!["checkpoint-put".to_string()];
    memory::push_optional_path_arg(
        &mut checkpoint_args,
        "--state",
        options.state,
        options.project_root,
    );
    memory::push_optional_string_arg(
        &mut checkpoint_args,
        "--embedding-dim",
        options.embedding_dim,
    );
    let worker_request = MemoryWorkerCheckpointPutRequest {
        command: "checkpoint-put",
        payload,
        state: options.state.map(|value| {
            memory::absolute_path(value, options.project_root)
                .display()
                .to_string()
        }),
        embedding_dim: options.embedding_dim,
    };
    let (receipt, transport) = if let Ok(socket_path) = env::var("ASP_MEMORY_ENGINE_SOCKET") {
        let output = memory::run_asp_memory_engine_worker(&socket_path, &worker_request)?;
        (decode_checkpoint_receipt(&output)?, "socket:env")
    } else if memory::memory_engine_auto_socket_enabled() {
        match memory::run_asp_memory_engine_auto_worker_checked(
            &worker_request,
            options.project_root,
            validate_checkpoint_receipt,
        ) {
            Ok(output) => (decode_checkpoint_receipt(&output)?, "socket:auto"),
            Err(_) => {
                let output = memory::run_asp_memory_engine(
                    &checkpoint_args,
                    &request_body,
                    options.project_root,
                )?;
                (decode_checkpoint_receipt(&output)?, "process:auto-fallback")
            }
        }
    } else {
        let output =
            memory::run_asp_memory_engine(&checkpoint_args, &request_body, options.project_root)?;
        (decode_checkpoint_receipt(&output)?, "process")
    };
    if let Some(error) = receipt.get("error") {
        return Err(format!("asp-memory-engine checkpoint-put failed: {error}"));
    }
    Ok(transport)
}

fn decode_checkpoint_receipt(output: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(output).map_err(|error| {
        format!("failed to decode asp-memory-engine checkpoint-put output: {error}")
    })
}

fn validate_checkpoint_receipt(output: &[u8]) -> Result<(), String> {
    let receipt = decode_checkpoint_receipt(output)?;
    if let Some(error) = receipt.get("error") {
        return Err(format!("asp-memory-engine checkpoint-put failed: {error}"));
    }
    Ok(())
}

fn checkpoint_payload(
    candidate: &OrgPlanCandidate,
    task: &OrgTaskCandidate,
    options: &CheckpointSyncOptions<'_>,
) -> CheckpointPutPayload {
    let source_locator = source_locator(&candidate.path, task);
    let plan_id = candidate.plan_id();
    CheckpointPutPayload {
        session_id: options.session.to_string(),
        title: task.title.clone(),
        status: "open",
        kind: task.kind.clone(),
        project_id: options.project.to_string(),
        plan_id: plan_id.clone(),
        branch_id: branch_id(candidate, options.branch),
        resume_command: format!("asp org query {plan_id} recovery evidence next-action"),
        source_locator,
        metadata: json!({
            "planTitle": candidate.title.clone(),
            "planPath": candidate.path.display().to_string(),
            "planTodo": candidate.todo.clone(),
            "planTodoType": candidate.todo_type.clone(),
            "taskStatus": task.status.clone(),
            "taskSection": task.section.clone(),
            "taskSourceLine": task.source_line,
        }),
    }
}

fn matches_session(candidate: &OrgPlanCandidate, session: &str) -> bool {
    candidate
        .properties
        .get("SESSION_ID")
        .is_some_and(|value| value == session)
}

fn branch_id(candidate: &OrgPlanCandidate, branch: Option<&str>) -> Option<String> {
    branch
        .map(str::to_string)
        .or_else(|| candidate.properties.get("BRANCH_ID").cloned())
        .or_else(|| candidate.properties.get("PLAN_BRANCH").cloned())
}

fn source_locator(path: &Path, task: &OrgTaskCandidate) -> String {
    task.source_line
        .map(|line| format!("{}:{line}-{line}", path.display()))
        .unwrap_or_else(|| path.display().to_string())
}
