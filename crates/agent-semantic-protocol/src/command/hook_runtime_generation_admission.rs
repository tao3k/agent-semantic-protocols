use std::path::PathBuf;

const WORKSPACE_ADMISSION_EVENTS: [&str; 2] = ["session-start", "user-prompt"];

pub(super) fn admit_hook_workspace_generation(args: &[String]) -> Result<(), String> {
    if !hook_event_requires_generation_admission(args) {
        return Ok(());
    }

    let project_root = hook_workspace_root()?;
    super::runtime_server::block_on_runtime_server_client(async move {
        let session =
            super::runtime_server::runtime_server_workspace_session_async(&project_root).await?;
        session
            .admit_runtime_generation(&project_root)
            .await
            .map(|_| ())
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
