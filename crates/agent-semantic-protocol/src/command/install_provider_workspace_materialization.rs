use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceDependencyMaterializationSpec {
    pub(super) program: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
    pub(super) working_directory: String,
    #[serde(default)]
    pub(super) env: BTreeMap<String, String>,
}

pub(super) fn resolve_workspace_relative_path(
    project_root: &Path,
    relative: &str,
    field: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "provider workspace build {field} must be a project-relative path without parent traversal: {relative}"
        ));
    }
    Ok(project_root.join(path))
}

pub(super) fn rendered_workspace_command_env(
    env: &BTreeMap<String, String>,
    workspace_root: &Path,
) -> BTreeMap<String, String> {
    let workspace_root = workspace_root.to_string_lossy();
    env.iter()
        .map(|(name, value)| {
            (
                name.clone(),
                value.replace("${ASP_WORKSPACE_ROOT}", &workspace_root),
            )
        })
        .collect()
}

pub(super) fn run_dependency_materialization(
    spec: &WorkspaceDependencyMaterializationSpec,
    provider_id: &str,
    project_root: &Path,
    sandbox_root: &Path,
) -> Result<(), String> {
    if spec.program.trim().is_empty() {
        return Err(format!(
            "provider {provider_id} dependencyMaterialization program must not be empty"
        ));
    }
    let program_name = Path::new(&spec.program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(spec.program.as_str());
    if matches!(program_name, "sh" | "bash" | "zsh" | "fish") {
        return Err(format!(
            "provider {provider_id} dependencyMaterialization must use an executable plus argv, not a shell interpreter"
        ));
    }
    let working_directory = resolve_workspace_relative_path(
        sandbox_root,
        &spec.working_directory,
        "dependencyMaterialization.workingDirectory",
    )?;
    if !working_directory.is_dir() {
        return Err(format!(
            "provider {provider_id} dependencyMaterialization working directory is missing at {}",
            working_directory.display()
        ));
    }
    let configured_program = Path::new(&spec.program);
    let program = if configured_program.is_absolute() {
        configured_program
            .strip_prefix(project_root)
            .map(|relative| sandbox_root.join(relative))
            .unwrap_or_else(|_| configured_program.to_path_buf())
    } else {
        configured_program.to_path_buf()
    };
    let mut command = std::process::Command::new(&program);
    command
        .args(&spec.args)
        .current_dir(&working_directory)
        .env("ASP_WORKSPACE_ROOT", sandbox_root)
        .envs(rendered_workspace_command_env(&spec.env, sandbox_root));
    let status = command.status().map_err(|error| {
        format!(
            "failed to start dependency materialization for provider {provider_id} with program `{}`: {error}",
            program.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "dependency materialization failed for provider {provider_id} with status {status}"
        ));
    }
    Ok(())
}
