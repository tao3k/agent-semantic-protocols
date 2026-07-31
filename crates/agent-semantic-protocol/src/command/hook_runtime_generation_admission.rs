use std::path::{Path, PathBuf};

const WORKSPACE_ADMISSION_EVENTS: [&str; 2] = ["session-start", "user-prompt"];

pub(super) fn admit_hook_workspace_generation(args: &[String]) -> Result<(), String> {
    if !hook_event_requires_generation_admission(args) {
        return Ok(());
    }

    let project_root = hook_workspace_root()?;
    request(&project_root).map(|_| ())
}

pub(super) fn request(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_async(&project_root)
                .await?;
        let receipt = session.admit_runtime_generation(&project_root).await?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode runtime generation admission: {error}"))
    })?
}

fn hook_event_requires_generation_admission(args: &[String]) -> bool {
    args.iter()
        .any(|argument| WORKSPACE_ADMISSION_EVENTS.contains(&argument.as_str()))
}

fn hook_workspace_root() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("failed to resolve hook workspace root: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission_tests;
