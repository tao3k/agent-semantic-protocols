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

pub(super) struct HookRuntimeBundleCandidate {
    pub source: PathBuf,
    pub artifact_digest: String,
}

pub(super) async fn admit_embedded_hook_runtime_candidate(
    installing_asp_binary: &Path,
) -> Result<HookRuntimeBundleCandidate, String> {
    admit_embedded_hook_config()?;
    agent_semantic_hook::aot_compiler::compile_embedded_hook_policy_bundle()
        .map_err(|error| format!("validate embedded Hook policy: {error}"))?;
    let source = resolve_hook_binary_candidate(installing_asp_binary)?;
    validate_hook_binary_candidate_identity(&source).await?;
    let artifact_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
            &source,
        )
        .await?
        .to_string();
    Ok(HookRuntimeBundleCandidate {
        source,
        artifact_digest,
    })
}

/// Retire the pre-Runtime Hook selector after the canonical evaluator is live.
///
/// `hooks/current` is never a serving authority.  Removing the legacy file or
/// symlink prevents operators and diagnostics from mistaking an abandoned
/// policy generation for the evaluator selected by the fixed plugin launcher.
/// A directory at this path is not an old selector and is preserved fail-closed.
pub(super) fn retire_legacy_hook_generation_pointer(
    protocol_home: &Path,
) -> Result<&'static str, String> {
    let legacy = protocol_home.join("hooks/current");
    match std::fs::symlink_metadata(&legacy) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            std::fs::remove_file(&legacy).map_err(|error| {
                format!(
                    "failed to retire legacy Hook generation pointer {}: {error}",
                    legacy.display()
                )
            })?;
            Ok("retired")
        }
        Ok(_) => Err(format!(
            "legacy-hook-generation-path-conflict: {} is not a file or symlink",
            legacy.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("absent"),
        Err(error) => Err(format!(
            "failed to inspect legacy Hook generation pointer {}: {error}",
            legacy.display()
        )),
    }
}

async fn validate_hook_binary_candidate_identity(candidate: &Path) -> Result<(), String> {
    let expected = agent_semantic_hook::aot_compiler::embedded_hook_policy_content_digest()?;
    let mut command = tokio::process::Command::new(candidate);
    command
        .arg("--identity")
        .env_remove("ASP_NO_AGENT")
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(1), command.output())
        .await
        .map_err(|_| {
            format!(
                "Hook binary candidate {} exceeded 1000ms identity timeout",
                candidate.display()
            )
        })?
        .map_err(|error| {
            format!(
                "failed to execute Hook binary candidate identity {}: {error}",
                candidate.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Hook binary candidate identity failed for {}: status={}",
            candidate.display(),
            output.status
        ));
    }
    let identity =
        serde_json::from_slice::<serde_json::Value>(&output.stdout).map_err(|error| {
            format!(
                "Hook binary candidate identity returned invalid JSON for {}: {error}",
                candidate.display()
            )
        })?;
    let actual = identity
        .get("policyContentDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "Hook binary candidate identity is incomplete for {}",
                candidate.display()
            )
        })?;
    if identity.get("schemaId").and_then(serde_json::Value::as_str)
        != Some("agent.semantic-protocols.hook-runtime-identity")
        || identity
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            != Some(1)
        || actual != expected
    {
        return Err(format!(
            "Hook binary candidate policy identity mismatch: candidate={} expected={} actual={actual}",
            candidate.display(),
            expected
        ));
    }
    Ok(())
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
