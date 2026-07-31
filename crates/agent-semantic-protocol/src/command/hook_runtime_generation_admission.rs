use std::path::Path;

const WORKSPACE_ADMISSION_EVENTS: [&str; 3] = ["session-start", "user-prompt", "pre-tool"];

pub(crate) fn request(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session.admit_runtime_generation().await?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode runtime generation admission: {error}"))
    })?
}

pub(super) fn hook_event_requires_generation_admission(args: &[String]) -> bool {
    args.iter()
        .any(|argument| WORKSPACE_ADMISSION_EVENTS.contains(&argument.as_str()))
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission_tests;
