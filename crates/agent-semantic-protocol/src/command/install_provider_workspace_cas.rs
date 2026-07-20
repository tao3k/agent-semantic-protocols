use std::{
    fs,
    path::{Path, PathBuf},
};

use super::install_provider::{
    MaterializedWorkspaceArtifact, WorkspaceBuildReceipt, resolve_workspace_relative_path,
};
use super::install_provider_archive::{install_executable_entrypoint, sha256_file};
use super::install_provider_release::ProviderReleaseSpec;
use super::install_provider_workspace_artifact::{
    WorkspaceArtifactLaunchSpec, capture_workspace_artifact_snapshot, copy_workspace_artifact_tree,
    remove_workspace_artifact_path,
};

#[derive(Debug)]
pub(super) struct InstalledWorkspaceEntrypoint {
    pub(super) cas_root: PathBuf,
    pub(super) cas_entrypoint: PathBuf,
    pub(super) installed_digest: String,
    pub(super) installed_sha256: String,
    pub(super) launcher_digest: Option<String>,
}

pub(super) fn install_workspace_artifact_from_cas(
    spec: &ProviderReleaseSpec,
    state: &agent_semantic_runtime::ProjectRuntimeState,
    artifact: &MaterializedWorkspaceArtifact,
    receipt: &WorkspaceBuildReceipt,
    runtime_artifact: &Path,
    install_target: &Path,
) -> Result<InstalledWorkspaceEntrypoint, String> {
    let cas_parent = state
        .protocol_home
        .join("runtime")
        .join("provider-artifacts")
        .join(&spec.language_id);
    fs::create_dir_all(&cas_parent)
        .map_err(|error| format!("failed to create {}: {error}", cas_parent.display()))?;
    let cas_root = cas_parent.join(&receipt.artifact_digest);
    materialize_workspace_artifact_cas(
        &artifact.workspace_root,
        &cas_root,
        &receipt.artifact_digest,
        receipt.artifact_leaf_count,
    )?;
    let cas_entrypoint = if artifact.workspace_root.is_file() {
        cas_root.clone()
    } else {
        cas_root.join(&artifact.entrypoint_relative)
    };
    if !cas_entrypoint.is_file() {
        return Err(format!(
            "immutable provider artifact entrypoint is missing at {}",
            cas_entrypoint.display()
        ));
    }
    let (installed_source, launcher_digest) = if let Some(launch) = &artifact.launch {
        let launcher = render_workspace_artifact_launcher(spec, &cas_root, launch)?;
        let launcher_digest = agent_semantic_content_identity::hash_blob(&launcher).value;
        let launcher_path = state
            .protocol_home
            .join("runtime")
            .join("provider-launchers")
            .join(&spec.language_id)
            .join(&launcher_digest)
            .join(&spec.binary);
        install_immutable_launcher(&launcher_path, &launcher)?;
        (launcher_path, Some(launcher_digest))
    } else {
        (cas_entrypoint.clone(), None)
    };
    install_executable_entrypoint(&installed_source, runtime_artifact)?;
    if install_target != runtime_artifact {
        install_executable_entrypoint(&installed_source, install_target)?;
    }
    let installed_bytes = fs::read(runtime_artifact).map_err(|error| {
        format!(
            "failed to read installed provider entrypoint {}: {error}",
            runtime_artifact.display()
        )
    })?;
    Ok(InstalledWorkspaceEntrypoint {
        cas_root,
        cas_entrypoint,
        installed_digest: agent_semantic_content_identity::hash_blob(&installed_bytes).value,
        installed_sha256: sha256_file(runtime_artifact)?,
        launcher_digest,
    })
}

pub(super) fn materialize_workspace_artifact_cas(
    source: &Path,
    cas_root: &Path,
    expected_root: &str,
    expected_leaf_count: usize,
) -> Result<(), String> {
    if fs::symlink_metadata(cas_root).is_ok() {
        if verify_workspace_artifact_cas(cas_root, expected_root, expected_leaf_count).is_ok() {
            return Ok(());
        }
        remove_workspace_artifact_path(cas_root)?;
    }
    let file_name = cas_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid provider artifact CAS path {}", cas_root.display()))?;
    let staging = cas_root.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    remove_workspace_artifact_path(&staging)?;
    copy_workspace_artifact_tree(source, &staging)?;
    verify_workspace_artifact_cas(&staging, expected_root, expected_leaf_count)?;
    match fs::rename(&staging, cas_root) {
        Ok(()) => {}
        Err(_error) if fs::symlink_metadata(cas_root).is_ok() => {
            remove_workspace_artifact_path(&staging)?;
            verify_workspace_artifact_cas(cas_root, expected_root, expected_leaf_count)?;
        }
        Err(error) => {
            remove_workspace_artifact_path(&staging)?;
            return Err(format!(
                "failed to publish immutable provider artifact {}: {error}",
                cas_root.display()
            ));
        }
    }
    verify_workspace_artifact_cas(cas_root, expected_root, expected_leaf_count)
}

fn verify_workspace_artifact_cas(
    cas_root: &Path,
    expected_root: &str,
    expected_leaf_count: usize,
) -> Result<(), String> {
    let snapshot = capture_workspace_artifact_snapshot(cas_root)?;
    if snapshot.root_digest != expected_root || snapshot.leaf_count != expected_leaf_count {
        return Err(format!(
            "immutable provider artifact CAS verification failed at {}: expectedRoot={} actualRoot={} expectedLeafCount={} actualLeafCount={}",
            cas_root.display(),
            expected_root,
            snapshot.root_digest,
            expected_leaf_count,
            snapshot.leaf_count
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn render_workspace_artifact_launcher(
    spec: &ProviderReleaseSpec,
    cas_root: &Path,
    launch: &WorkspaceArtifactLaunchSpec,
) -> Result<Vec<u8>, String> {
    if launch.program.trim().is_empty() {
        return Err(format!(
            "provider {} workspaceArtifact.launch.program must not be empty",
            spec.language_id
        ));
    }
    let program_name = Path::new(&launch.program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(launch.program.as_str());
    if matches!(program_name, "sh" | "bash" | "zsh" | "fish") {
        return Err(format!(
            "provider {} workspaceArtifact.launch must name the provider runtime, not a command shell",
            spec.language_id
        ));
    }
    let program = if launch.program_relative_to_artifact {
        let path = resolve_workspace_relative_path(
            cas_root,
            &launch.program,
            "workspaceArtifact.launch.program",
        )?;
        if !path.is_file() {
            return Err(format!(
                "provider {} artifact-relative launch program is missing at {}",
                spec.language_id,
                path.display()
            ));
        }
        path.display().to_string()
    } else {
        launch.program.clone()
    };
    let mut command = vec![shell_quote(&program)];
    for arg in &launch.args {
        let value = if launch.args_relative_to_artifact {
            let path =
                resolve_workspace_relative_path(cas_root, arg, "workspaceArtifact.launch.args")?;
            if !path.exists() {
                return Err(format!(
                    "provider {} artifact-relative launch argument is missing at {}",
                    spec.language_id,
                    path.display()
                ));
            }
            path.display().to_string()
        } else {
            arg.clone()
        };
        command.push(shell_quote(&value));
    }
    Ok(format!("#!/bin/sh\nexec {} \"$@\"\n", command.join(" ")).into_bytes())
}

#[cfg(not(unix))]
fn render_workspace_artifact_launcher(
    spec: &ProviderReleaseSpec,
    _cas_root: &Path,
    _launch: &WorkspaceArtifactLaunchSpec,
) -> Result<Vec<u8>, String> {
    Err(format!(
        "provider {} workspaceArtifact.launch is not supported on this target",
        spec.language_id
    ))
}

fn install_immutable_launcher(path: &Path, contents: &[u8]) -> Result<(), String> {
    if path.is_file() {
        let existing = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if existing != contents {
            return Err(format!(
                "immutable provider launcher digest collision at {}",
                path.display()
            ));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid provider launcher path {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staging = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&staging, contents)
        .map_err(|error| format!("failed to write {}: {error}", staging.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("failed to chmod {}: {error}", staging.display()))?;
    }
    match fs::rename(&staging, path) {
        Ok(()) => Ok(()),
        Err(_) if path.is_file() => {
            let _ = fs::remove_file(&staging);
            let existing = fs::read(path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            if existing == contents {
                Ok(())
            } else {
                Err(format!(
                    "immutable provider launcher digest collision at {}",
                    path.display()
                ))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(format!(
                "failed to publish immutable provider launcher {}: {error}",
                path.display()
            ))
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
