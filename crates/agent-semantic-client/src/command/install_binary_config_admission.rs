use std::path::{Path, PathBuf};

pub(super) fn admit_embedded_hook_config() -> Result<(), String> {
    agent_semantic_config::default_hook_client_config_file()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "ASP binary/config publication admission failed before artifact switch: {error}"
            )
        })
}

/// Materialize the human-readable operator copy of the embedded Hook config.
///
/// Serving evaluation never reads this path. Matcher, policy and Agent
/// registry bytes are compiled into the canonical `runtime/bin/asp-hook`
/// executable.
pub(super) fn publish_embedded_hook_config(protocol_home: &Path) -> Result<&'static str, String> {
    let path = protocol_home.join("hooks/config.toml");
    let status = super::managed_hook_config::materialize(&path).map_err(|error| {
        format!(
            "ASP binary/config publication failed for {}: {error}",
            path.display()
        )
    })?;
    Ok(status.as_str())
}

pub(super) struct HookRuntimeInstallReceipt {
    pub path: PathBuf,
    pub artifact_digest: String,
    pub lock_elapsed_micros: u128,
    pub config_source_status: &'static str,
}

/// Publish the dedicated Rust Hook evaluator into the canonical Runtime bin.
///
/// The plugin launcher is a fixed shell entrypoint. It never owns a binary
/// generation and always resolves this stable Runtime path.
pub(super) async fn publish_embedded_hook_runtime(
    protocol_home: &Path,
    installing_asp_binary: &Path,
) -> Result<HookRuntimeInstallReceipt, String> {
    admit_embedded_hook_config()?;
    agent_semantic_hook::aot_compiler::compile_embedded_hook_policy_bundle()
        .map_err(|error| format!("validate embedded Hook policy: {error}"))?;
    let hook_binary = resolve_hook_binary_candidate(installing_asp_binary)?;
    let target = protocol_home.join("runtime/bin/asp-hook");
    let publication =
        agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_tool_artifact(
            protocol_home,
            &hook_binary,
            &target,
            "asp-hook",
        )
        .await?;
    let config_source_status = publish_embedded_hook_config(protocol_home)?;
    Ok(HookRuntimeInstallReceipt {
        path: publication.path,
        artifact_digest: publication.artifact_digest.to_string(),
        lock_elapsed_micros: publication.lock_elapsed_micros,
        config_source_status,
    })
}

fn resolve_hook_binary_candidate(
    installing_asp_binary: &Path,
) -> Result<std::path::PathBuf, String> {
    resolve_executable_sibling(installing_asp_binary, "asp-hook", "Hook binary")
}

fn resolve_executable_sibling(
    installing_asp_binary: &Path,
    file_name: &str,
    label: &str,
) -> Result<std::path::PathBuf, String> {
    let parent = installing_asp_binary.parent().ok_or_else(|| {
        format!(
            "installing ASP binary has no build directory: {}",
            installing_asp_binary.display()
        )
    })?;
    let candidate = parent.join(file_name);
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        format!(
            "{label} candidate is unavailable at {}: {error}; build the Hook package binary in the same target directory before installation",
            candidate.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(format!(
                "{label} candidate is not an executable regular file: {}",
                candidate.display()
            ));
        }
    }
    Ok(candidate)
}

#[cfg(test)]
#[path = "../../tests/unit/install_binary_config_admission.rs"]
mod tests;
