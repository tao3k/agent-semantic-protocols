use std::path::Path;

const WORKSPACE_ADMISSION_EVENTS: [&str; 2] = ["session-start", "user-prompt"];

#[derive(Debug, Default)]
pub(super) struct HookGenerationAdmissionObservation {
    pub(super) receipt: Option<serde_json::Value>,
    pub(super) error: Option<String>,
}

pub(super) fn decision_changed_paths(decision: &agent_semantic_hook::HookDecision) -> Vec<String> {
    let mut changed_paths = decision
        .fields
        .get("normalizedActions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|action| {
            action
                .get("operationIntent")
                .and_then(serde_json::Value::as_str)
                == Some("apply-patch")
        })
        .flat_map(|action| {
            action
                .get("paths")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if decision
        .fields
        .get("operationIntent")
        .and_then(serde_json::Value::as_str)
        == Some("apply-patch")
    {
        changed_paths.extend(
            decision
                .fields
                .get("paths")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .filter(|path| !path.trim().is_empty())
                .map(str::to_owned),
        );
    }
    changed_paths.sort();
    changed_paths.dedup();
    changed_paths
}

pub(super) fn payload_mutation_id(payload: &serde_json::Value) -> Option<String> {
    let session_id = payload.get("session_id")?.as_str()?.trim();
    let tool_use_id = payload.get("tool_use_id")?.as_str()?.trim();
    if session_id.is_empty() || tool_use_id.is_empty() {
        return None;
    }
    Some(format!("{session_id}/{tool_use_id}"))
}

pub(crate) fn request(
    project_root: &Path,
    mutation_id: String,
    changed_paths: Vec<String>,
) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session
            .submit_runtime_generation_mutation(mutation_id, changed_paths)
            .await?;
        receipt.validate()?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode runtime generation admission: {error}"))
    })?
}

pub(super) fn hook_event_requires_generation_admission(
    args: &[String],
    workspace_mutated: bool,
    explicit_asp_workspace: bool,
) -> bool {
    args.iter().any(|argument| {
        WORKSPACE_ADMISSION_EVENTS.contains(&argument.as_str())
            || (argument == "post-tool" && workspace_mutated)
            || (argument == "pre-tool" && explicit_asp_workspace)
    })
}

pub(super) fn decision_mutates_workspace(decision: &agent_semantic_hook::HookDecision) -> bool {
    decision
        .fields
        .get("operationIntent")
        .and_then(serde_json::Value::as_str)
        == Some("apply-patch")
        || decision
            .fields
            .get("normalizedActions")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|actions| {
                actions.iter().any(|action| {
                    action
                        .get("operationIntent")
                        .and_then(serde_json::Value::as_str)
                        == Some("apply-patch")
                })
            })
}

pub(crate) fn ensure(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session.ensure_runtime_generation().await?;
        receipt.validate()?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode runtime generation ensure: {error}"))
    })?
}

/// Establish terminal workspace-generation readiness before an explicit ASP
/// search/query command enters the read-only data plane.
pub(crate) fn ensure_ready(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session.repair_runtime_generation_locator().await?;
        receipt.validate()?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode ready runtime generation: {error}"))
    })?
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod tests;

pub(super) fn observe(
    args: &[String],
    workspace_mutated: bool,
    mutation_id: Option<String>,
    changed_paths: Vec<String>,
    explicit_asp_workspace: bool,
    admit: impl FnOnce(String, Vec<String>) -> Result<serde_json::Value, String>,
    ensure: impl FnOnce() -> Result<serde_json::Value, String>,
) -> HookGenerationAdmissionObservation {
    if !hook_event_requires_generation_admission(args, workspace_mutated, explicit_asp_workspace) {
        return HookGenerationAdmissionObservation::default();
    }

    let result = if workspace_mutated {
        if mutation_id.is_none() {
            Err("post-tool workspace mutation omitted typed mutation identity".to_owned())
        } else if changed_paths.is_empty() {
            Err("post-tool workspace mutation omitted normalized changed paths".to_owned())
        } else {
            admit(
                mutation_id.expect("validated mutation identity"),
                changed_paths,
            )
        }
    } else {
        ensure()
    };
    match result {
        Ok(receipt) => HookGenerationAdmissionObservation {
            receipt: Some(receipt),
            error: None,
        },
        Err(error) => HookGenerationAdmissionObservation {
            receipt: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission_tests;
