// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::Path;
use std::path::PathBuf;

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
/// registry bytes are compiled into the `active/asp-hook` member of the sole
/// verified Runtime artifact bundle.
pub(in crate::command) fn publish_embedded_hook_config(
    protocol_home: &Path,
) -> Result<&'static str, String> {
    let path = agent_semantic_artifacts::StateHomeLayout::new(protocol_home)
        .control()
        .hook_client_config();
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
    let artifact_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
            &source,
        )
        .await?
        .to_string();
    // The immutable Runtime bundle binds this complete artifact digest to the
    // `asp-hook` member and derives one bundle digest over every member.  Do
    // not execute a candidate during publication admission: process creation,
    // loader and code-signature latency are neither semantic identity nor a
    // deterministic part of the publication transaction.  `--identity`
    // remains a runtime diagnostic only; Active/Healthy bundle content is the
    // sole installation authority.
    Ok(HookRuntimeBundleCandidate {
        source,
        artifact_digest,
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
