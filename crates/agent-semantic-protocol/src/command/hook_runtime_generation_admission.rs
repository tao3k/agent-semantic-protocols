use std::path::Path;

const WORKSPACE_ADMISSION_EVENTS: [&str; 2] = ["session-start", "user-prompt"];

#[derive(Debug, Default)]
pub(in crate::command) struct HookGenerationAdmissionObservation {
    pub(super) requested: bool,
    pub(super) receipt: Option<serde_json::Value>,
    pub(super) error: Option<String>,
}

pub(super) fn materialize_explicit_query_gate(
    decision: &mut agent_semantic_hook::HookDecision,
    explicit_asp_workspace: bool,
    observation: HookGenerationAdmissionObservation,
) {
    decision.fields.insert(
        "runtimeGenerationAdmissionStatus".to_owned(),
        serde_json::Value::String(
            if observation.requested {
                if observation.error.is_some() {
                    if explicit_asp_workspace {
                        "failed-closed"
                    } else {
                        "failed-non-blocking"
                    }
                } else {
                    "submitted"
                }
            } else {
                "not-requested"
            }
            .to_owned(),
        ),
    );
    if let Some(receipt) = observation.receipt {
        decision
            .fields
            .insert("runtimeGenerationAdmission".to_owned(), receipt);
    }
    if let Some(error) = observation.error {
        decision.fields.insert(
            "runtimeGenerationAdmissionError".to_owned(),
            serde_json::Value::String(error.clone()),
        );
        if explicit_asp_workspace {
            decision.decision = agent_semantic_hook::DecisionKind::Block;
            decision.reason_kind = agent_semantic_hook::ReasonKind::ActivationUnavailable;
            decision.message = format!(
                "ASP blocked the explicit search/query because its workspace generation did not reach terminal Ready: {error}"
            );
        }
    }
}

pub(super) fn payload_mutation_id(payload: &serde_json::Value) -> Option<String> {
    let session_id = payload.get("session_id")?.as_str()?.trim();
    let tool_use_id = payload.get("tool_use_id")?.as_str()?.trim();
    if session_id.is_empty() || tool_use_id.is_empty() {
        return None;
    }
    Some(format!("{session_id}/{tool_use_id}"))
}

pub(super) fn payload_changed_paths(payload: &serde_json::Value) -> Vec<String> {
    let Some(tool_name) = payload.get("tool_name").and_then(serde_json::Value::as_str) else {
        return Vec::new();
    };
    let tool_input = payload
        .get("tool_input")
        .unwrap_or(&serde_json::Value::Null);
    agent_semantic_hook::workspace_mutation_paths(tool_name, tool_input)
}

pub(crate) fn request(
    project_root: &Path,
    mutation_id: String,
    changed_paths: Vec<String>,
) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::server::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::server::runtime_server::runtime_server_workspace_session_for_admission_async(
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
    event: &str,
    workspace_mutated: bool,
    explicit_asp_workspace: bool,
) -> bool {
    WORKSPACE_ADMISSION_EVENTS.contains(&event)
        || (event == "post-tool" && workspace_mutated)
        || (event == "pre-tool" && explicit_asp_workspace)
}

pub(crate) fn ensure(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::server::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::server::runtime_server::runtime_server_workspace_session_for_admission_async(
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
    crate::server::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::server::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session.ensure_runtime_generation_ready().await?;
        receipt.validate()?;
        serde_json::to_value(receipt).map_err(|error| {
            format!("failed to encode ready runtime generation observation: {error}")
        })
    })?
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod tests;

pub(super) fn observe(
    event: &str,
    workspace_mutated: bool,
    mutation_id: Option<String>,
    changed_paths: Vec<String>,
    explicit_asp_workspace: bool,
    admit: impl FnOnce(String, Vec<String>) -> Result<serde_json::Value, String>,
    ensure: impl FnOnce() -> Result<serde_json::Value, String>,
) -> HookGenerationAdmissionObservation {
    if !hook_event_requires_generation_admission(event, workspace_mutated, explicit_asp_workspace) {
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
            requested: true,
            receipt: Some(receipt),
            error: None,
        },
        Err(error) => HookGenerationAdmissionObservation {
            requested: true,
            receipt: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission_tests;
