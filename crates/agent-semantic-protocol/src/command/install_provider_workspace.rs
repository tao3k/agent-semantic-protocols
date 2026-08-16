//! Registry-driven development provider workspace build and artifact publication.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessSpec, ProviderProcessSupervisor, StdinMode,
    provider_process_limits_from_environment,
};
use serde::Deserialize;

#[path = "install_provider_workspace_receipt.rs"]
mod receipt;
pub(super) use receipt::record_registered_provider_workspace_install;

const DEFAULT_WORKSPACE_BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderWorkspaceInstallDescriptor {
    schema_id: String,
    schema_version: String,
    schema_authority: String,
    provider_id: String,
    binary: String,
    workspace_artifact: WorkspaceArtifactDescriptor,
    dependency_materialization: Option<WorkspaceCommandDescriptor>,
    workspace_build: WorkspaceBuildDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceArtifactDescriptor {
    root: String,
    entrypoint: String,
    launch: Option<WorkspaceLaunchDescriptor>,
    #[serde(default)]
    runtime_dependencies: Vec<WorkspaceRuntimeDependencyDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceRuntimeDependencyDescriptor {
    source: String,
    target: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceLaunchDescriptor {
    program: String,
    args: Vec<String>,
    program_relative_to_artifact: bool,
    args_relative_to_artifact: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceBuildDescriptor {
    program: String,
    args: Vec<String>,
    working_directory: String,
    source_snapshot_anchors: Vec<String>,
    derived_paths: Vec<String>,
    env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceCommandDescriptor {
    program: String,
    args: Vec<String>,
    working_directory: String,
    env: BTreeMap<String, String>,
}

pub(super) struct BuiltProviderWorkspace {
    source_root: PathBuf,
    entrypoint: PathBuf,
    launch: Option<WorkspaceLaunchDescriptor>,
    runtime_dependencies: Vec<BuiltWorkspaceRuntimeDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuiltWorkspaceRuntimeDependency {
    source: PathBuf,
    target: PathBuf,
}

pub(super) struct PublishedProviderWorkspace {
    pub(super) source_root: PathBuf,
    pub(super) artifact_root: PathBuf,
    pub(super) artifact_entrypoint: PathBuf,
    pub(super) artifact_digest: String,
    pub(super) artifact_leaf_count: usize,
    pub(super) launcher: PathBuf,
    pub(super) installed_path: PathBuf,
}

pub(super) async fn build_registered_provider_workspace(
    configured_dev_root: &Path,
    registration: &agent_semantic_hook::ProviderDevelopmentRegistrationV1,
) -> Result<BuiltProviderWorkspace, String> {
    let dev_root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    let provider_source_root = dev_root
        .join(&registration.development.source_root)
        .canonicalize()
        .map_err(|error| format!("canonicalize provider sourceRoot: {error}"))?;
    ensure_within(&provider_source_root, &dev_root, "provider sourceRoot")?;
    let reference = registration
        .development
        .workspace_install
        .as_deref()
        .ok_or_else(|| "registered provider lacks development.workspaceInstall".to_string())?;
    validate_relative_path(Path::new(reference), false, "workspaceInstall")?;
    let descriptor_path = provider_source_root
        .join(reference)
        .canonicalize()
        .map_err(|error| format!("canonicalize workspace install descriptor: {error}"))?;
    ensure_within(
        &descriptor_path,
        &provider_source_root,
        "workspace install descriptor",
    )?;
    let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_slice(
        &fs::read(&descriptor_path)
            .map_err(|error| format!("read {}: {error}", descriptor_path.display()))?,
    )
    .map_err(|error| format!("decode {}: {error}", descriptor_path.display()))?;
    validate_descriptor(&descriptor, registration)?;

    let working_directory = repository_path(
        &dev_root,
        &descriptor.workspace_build.working_directory,
        true,
        "workspaceBuild.workingDirectory",
    )?;
    for anchor in &descriptor.workspace_build.source_snapshot_anchors {
        let path = repository_path(
            &dev_root,
            anchor,
            false,
            "workspaceBuild.sourceSnapshotAnchors",
        )?;
        if !path.exists() {
            return Err(format!(
                "workspace build source anchor is missing: {}",
                path.display()
            ));
        }
    }
    let artifact_path = repository_path(
        &dev_root,
        &descriptor.workspace_artifact.root,
        false,
        "workspaceArtifact.root",
    )?;
    let artifact_is_declared = descriptor.workspace_build.derived_paths.iter().any(|path| {
        repository_path(&dev_root, path, false, "workspaceBuild.derivedPaths").is_ok_and(
            |candidate| artifact_path == candidate || artifact_path.starts_with(candidate),
        )
    });
    if !artifact_is_declared {
        return Err(format!(
            "workspace artifact root is not declared in workspaceBuild.derivedPaths: {}",
            artifact_path.display()
        ));
    }
    if let Some(materialization) = descriptor.dependency_materialization.as_ref() {
        let materialization_cwd = repository_path(
            &dev_root,
            &materialization.working_directory,
            true,
            "dependencyMaterialization.workingDirectory",
        )?;
        run_workspace_command(
            "dependency-materialization",
            registration,
            &dev_root,
            &materialization.program,
            &materialization.args,
            materialization_cwd,
            &materialization.env,
        )
        .await?;
    }
    run_workspace_command(
        "workspace-build",
        registration,
        &dev_root,
        &descriptor.workspace_build.program,
        &descriptor.workspace_build.args,
        working_directory,
        &descriptor.workspace_build.env,
    )
    .await?;
    let source_root = artifact_path
        .canonicalize()
        .map_err(|error| format!("canonicalize built workspace artifact: {error}"))?;
    ensure_within(&source_root, &dev_root, "built workspace artifact")?;
    let entrypoint = PathBuf::from(&descriptor.workspace_artifact.entrypoint);
    validate_relative_path(&entrypoint, true, "workspaceArtifact.entrypoint")?;
    let source_entrypoint = if source_root.is_file() && entrypoint == Path::new(".") {
        source_root.clone()
    } else {
        source_root.join(&entrypoint)
    };
    if !source_entrypoint.is_file() {
        return Err(format!(
            "built workspace artifact entrypoint is not a file: {}",
            source_entrypoint.display()
        ));
    }
    let runtime_dependencies = resolve_runtime_dependencies(
        &source_root,
        descriptor.workspace_artifact.launch.as_ref(),
        &descriptor.workspace_artifact.runtime_dependencies,
    )?;
    Ok(BuiltProviderWorkspace {
        source_root,
        entrypoint,
        launch: descriptor.workspace_artifact.launch,
        runtime_dependencies,
    })
}

fn resolve_runtime_dependencies(
    source_root: &Path,
    launch: Option<&WorkspaceLaunchDescriptor>,
    dependencies: &[WorkspaceRuntimeDependencyDescriptor],
) -> Result<Vec<BuiltWorkspaceRuntimeDependency>, String> {
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let launch = launch.ok_or_else(|| {
        "workspaceArtifact.runtimeDependencies requires workspaceArtifact.launch".to_owned()
    })?;
    if !launch.program_relative_to_artifact {
        return Err(
            "workspaceArtifact.runtimeDependencies requires an artifact-relative launch program"
                .to_owned(),
        );
    }
    let launch_program = Path::new(&launch.program);
    validate_relative_path(launch_program, false, "workspaceArtifact.launch.program")?;
    let resolved_launch_program = source_root
        .join(launch_program)
        .canonicalize()
        .map_err(|error| format!("resolve workspace artifact launch program: {error}"))?;
    if !resolved_launch_program.is_file() {
        return Err(format!(
            "resolved workspace artifact launch program is not a file: {}",
            resolved_launch_program.display()
        ));
    }
    let launch_bin = resolved_launch_program.parent().ok_or_else(|| {
        format!(
            "resolved workspace artifact launch program has no parent: {}",
            resolved_launch_program.display()
        )
    })?;
    let launch_prefix = launch_bin.parent().ok_or_else(|| {
        format!(
            "resolved workspace artifact launch program has no installation prefix: {}",
            resolved_launch_program.display()
        )
    })?;
    dependencies
        .iter()
        .map(|dependency| {
            let source = Path::new(&dependency.source);
            validate_relative_path(
                source,
                false,
                "workspaceArtifact.runtimeDependencies.source",
            )?;
            let source = launch_prefix.join(source).canonicalize().map_err(|error| {
                format!(
                    "resolve workspace artifact runtime dependency {}: {error}",
                    dependency.source
                )
            })?;
            ensure_within(
                &source,
                launch_prefix,
                "workspace artifact runtime dependency",
            )?;
            if !source.is_file() {
                return Err(format!(
                    "workspace artifact runtime dependency is not a file: {}",
                    source.display()
                ));
            }
            let target = PathBuf::from(&dependency.target);
            validate_relative_path(
                &target,
                false,
                "workspaceArtifact.runtimeDependencies.target",
            )?;
            Ok(BuiltWorkspaceRuntimeDependency { source, target })
        })
        .collect()
}

async fn run_workspace_command(
    stage: &str,
    registration: &agent_semantic_hook::ProviderDevelopmentRegistrationV1,
    dev_root: &Path,
    program: &str,
    args: &[String],
    cwd: PathBuf,
    environment: &BTreeMap<String, String>,
) -> Result<(), String> {
    let env = environment
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.replace("${ASP_WORKSPACE_ROOT}", &dev_root.to_string_lossy()),
            )
        })
        .collect();
    let limits = provider_process_limits_from_environment()?.with_workspace_build_memory_budget();
    let supervisor = ProviderProcessSupervisor::default();
    let result = supervisor
        .run(ProviderProcessSpec {
            program: program.to_string(),
            args: args.to_vec(),
            cwd,
            env,
            stdin: StdinMode::Inherit,
            stdout: OutputMode::Tee,
            stderr: OutputMode::Tee,
            limits: if limits.timeout().is_some() {
                limits
            } else {
                limits.with_timeout(Some(DEFAULT_WORKSPACE_BUILD_TIMEOUT))
            },
        })
        .await
        .map_err(|error| {
            format!(
                "registered provider {stage} gate failed: language={} provider={} error={error}",
                registration.language_id.as_str(),
                registration.provider_id.as_str()
            )
        });
    supervisor.shutdown().await;
    let output = result?;
    if !output.status.success() {
        return Err(format!(
            "registered provider {stage} failed: language={} provider={} status={}",
            registration.language_id.as_str(),
            registration.provider_id.as_str(),
            output.status
        ));
    }
    Ok(())
}

pub(super) fn publish_provider_workspace(
    protocol_home: &Path,
    stable_entry: &Path,
    binary_artifact_root: &Path,
    registration: &agent_semantic_hook::ProviderDevelopmentRegistrationV1,
    built: BuiltProviderWorkspace,
) -> Result<PublishedProviderWorkspace, String> {
    let publication_root = protocol_home
        .join("runtime/provider-artifacts")
        .join(&registration.binary)
        .join("artifacts/blake3-merkle-v1");
    fs::create_dir_all(&publication_root)
        .map_err(|error| format!("create {}: {error}", publication_root.display()))?;
    let stage = publication_root.join(format!(
        ".workspace.{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("read provider publication clock: {error}"))?
            .as_nanos()
    ));
    fs::create_dir(&stage).map_err(|error| {
        format!(
            "create provider artifact stage {}: {error}",
            stage.display()
        )
    })?;
    let staged_root = stage.join("root");
    copy_artifact_root(&built.source_root, &staged_root)?;
    materialize_runtime_dependencies(&staged_root, &built.runtime_dependencies)?;
    let (artifact_digest, artifact_leaf_count) = artifact_snapshot(&staged_root)?;
    let digest_leaf = artifact_digest
        .strip_prefix("blake3-256:")
        .unwrap_or(&artifact_digest);
    if digest_leaf.len() != 64 || !digest_leaf.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "unsupported workspace artifact digest: {artifact_digest}"
        ));
    }
    let artifact_dir = publication_root.join(digest_leaf);
    let artifact_root = artifact_dir.join("root");
    let artifact_entrypoint = if built.source_root.is_file() && built.entrypoint == Path::new(".") {
        artifact_root.clone()
    } else {
        artifact_root.join(&built.entrypoint)
    };
    let launcher = artifact_dir.join("launcher");
    let expected_launcher =
        launcher_script(&artifact_root, &artifact_entrypoint, built.launch.as_ref())?;
    if artifact_dir.exists() {
        let (published_digest, published_leaf_count) = artifact_snapshot(&artifact_root)?;
        if published_digest != artifact_digest || published_leaf_count != artifact_leaf_count {
            return Err(format!(
                "published provider workspace artifact drift: path={} expected={} actual={}",
                artifact_root.display(),
                artifact_digest,
                published_digest
            ));
        }
        let actual_launcher = fs::read_to_string(&launcher)
            .map_err(|error| format!("read immutable launcher {}: {error}", launcher.display()))?;
        if actual_launcher != expected_launcher {
            return Err(format!(
                "published provider workspace launcher drift: {}",
                launcher.display()
            ));
        }
        fs::remove_dir_all(&stage).map_err(|error| {
            format!(
                "remove redundant provider artifact stage {}: {error}",
                stage.display()
            )
        })?;
    } else {
        let staged_launcher = stage.join("launcher");
        fs::write(&staged_launcher, expected_launcher.as_bytes())
            .map_err(|error| format!("write {}: {error}", staged_launcher.display()))?;
        set_executable(&staged_launcher)?;
        fs::rename(&stage, &artifact_dir).map_err(|error| {
            format!(
                "atomically publish provider workspace artifact {}: {error}",
                artifact_dir.display()
            )
        })?;
    }
    let binary_identity =
        super::super::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(
            &registration.binary,
        )?;
    let installed = super::super::protocol_binary::install_protocol_binary_target(
        &launcher,
        stable_entry,
        binary_artifact_root,
        &binary_identity,
    )?;
    Ok(PublishedProviderWorkspace {
        source_root: built.source_root,
        artifact_root,
        artifact_entrypoint,
        artifact_digest,
        artifact_leaf_count,
        launcher,
        installed_path: installed.path,
    })
}

fn materialize_runtime_dependencies(
    staged_root: &Path,
    dependencies: &[BuiltWorkspaceRuntimeDependency],
) -> Result<(), String> {
    if dependencies.is_empty() {
        return Ok(());
    }
    if !staged_root.is_dir() {
        return Err(
            "workspace artifact runtime dependencies require a directory artifact".to_owned(),
        );
    }
    for dependency in dependencies {
        let target = staged_root.join(&dependency.target);
        if target.exists() {
            return Err(format!(
                "workspace artifact runtime dependency target already exists: {}",
                target.display()
            ));
        }
        let parent = target.parent().ok_or_else(|| {
            format!(
                "workspace artifact runtime dependency target has no parent: {}",
                target.display()
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create workspace artifact runtime dependency directory {}: {error}",
                parent.display()
            )
        })?;
        fs::copy(&dependency.source, &target).map_err(|error| {
            format!(
                "copy workspace artifact runtime dependency {} to {}: {error}",
                dependency.source.display(),
                target.display()
            )
        })?;
        fs::set_permissions(
            &target,
            fs::metadata(&dependency.source)
                .map_err(|error| {
                    format!(
                        "inspect workspace artifact runtime dependency {}: {error}",
                        dependency.source.display()
                    )
                })?
                .permissions(),
        )
        .map_err(|error| {
            format!(
                "copy workspace artifact runtime dependency permissions {}: {error}",
                target.display()
            )
        })?;
    }
    Ok(())
}

fn validate_descriptor(
    descriptor: &ProviderWorkspaceInstallDescriptor,
    registration: &agent_semantic_hook::ProviderDevelopmentRegistrationV1,
) -> Result<(), String> {
    if descriptor.schema_id != "agent.semantic-protocols.provider-workspace-install"
        || descriptor.schema_version != "1"
        || descriptor.schema_authority
            != "https://tao3k.github.io/agent-semantic-protocols/schemas/"
    {
        return Err("provider workspace install schema identity must be version 1".to_string());
    }
    if descriptor.provider_id != registration.provider_id.as_str()
        || descriptor.binary != registration.binary
    {
        return Err(format!(
            "provider workspace install identity drift: provider={} binary={} expectedProvider={} expectedBinary={}",
            descriptor.provider_id,
            descriptor.binary,
            registration.provider_id.as_str(),
            registration.binary
        ));
    }
    if descriptor
        .workspace_build
        .source_snapshot_anchors
        .is_empty()
        || descriptor.workspace_build.derived_paths.is_empty()
    {
        return Err(
            "provider workspace build requires source anchors and derived paths".to_string(),
        );
    }
    Ok(())
}

fn repository_path(
    root: &Path,
    value: &str,
    require_directory: bool,
    field: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(value);
    validate_relative_path(relative, true, field)?;
    let path = root.join(relative);
    if require_directory && !path.is_dir() {
        return Err(format!("{field} is not a directory: {}", path.display()));
    }
    Ok(path)
}

fn validate_relative_path(path: &Path, allow_dot: bool, field: &str) -> Result<(), String> {
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || (!allow_dot && path == Path::new("."))
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{field} must be a repository-relative path: {}",
            path.display()
        ));
    }
    Ok(())
}

fn ensure_within(path: &Path, root: &Path, label: &str) -> Result<(), String> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(format!(
            "{label} escaped registered root: {}",
            path.display()
        ))
    }
}

fn artifact_snapshot(root: &Path) -> Result<(String, usize), String> {
    let mut leaves = Vec::new();
    if root.is_file() {
        let leaf_name = root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                format!(
                    "provider workspace file artifact has no normalized UTF-8 leaf name: {}",
                    root.display()
                )
            })?;
        leaves.push((
            leaf_name.to_owned(),
            agent_semantic_content_identity::file_content_digest_v1(root)?,
        ));
    } else if root.is_dir() {
        collect_artifact_leaves(root, root, &mut leaves)?;
    } else {
        return Err(format!(
            "provider workspace artifact is not file or directory: {}",
            root.display()
        ));
    }
    if leaves.is_empty() {
        return Err(format!(
            "provider workspace artifact has no files: {}",
            root.display()
        ));
    }
    leaves.sort_by(|left, right| left.0.cmp(&right.0));
    let leaf_count = leaves.len();
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(leaves);
    Ok((snapshot.root_digest().to_string(), leaf_count))
}

fn collect_artifact_leaves(
    root: &Path,
    directory: &Path,
    leaves: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "read provider artifact directory {}: {error}",
                directory.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read provider artifact entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let link_metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect provider artifact {}: {error}", path.display()))?;
        if link_metadata.file_type().is_symlink() {
            let target_metadata = fs::metadata(&path).map_err(|error| {
                format!(
                    "resolve provider artifact symlink {}: {error}",
                    path.display()
                )
            })?;
            if target_metadata.is_dir() {
                return Err(format!(
                    "provider workspace artifact contains directory symlink: {}",
                    path.display()
                ));
            }
        } else if link_metadata.is_dir() {
            collect_artifact_leaves(root, &path, leaves)?;
            continue;
        } else if !link_metadata.is_file() {
            return Err(format!(
                "provider workspace artifact contains unsupported entry: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("derive provider artifact relative path: {error}"))?
            .to_string_lossy()
            .into_owned();
        leaves.push((
            relative,
            agent_semantic_content_identity::file_content_digest_v1(&path)?,
        ));
    }
    Ok(())
}

fn copy_artifact_root(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_file() {
        fs::copy(source, target)
            .map_err(|error| format!("copy provider artifact {}: {error}", source.display()))?;
        fs::set_permissions(
            target,
            fs::metadata(source)
                .map_err(|error| error.to_string())?
                .permissions(),
        )
        .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        return Ok(());
    }
    fs::create_dir(target).map_err(|error| {
        format!(
            "create provider artifact root {}: {error}",
            target.display()
        )
    })?;
    copy_artifact_directory(source, target)
}

fn copy_artifact_directory(source: &Path, target: &Path) -> Result<(), String> {
    let mut entries = fs::read_dir(source)
        .map_err(|error| {
            format!(
                "read provider artifact directory {}: {error}",
                source.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read provider artifact entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let link_metadata = fs::symlink_metadata(&source_path).map_err(|error| {
            format!(
                "inspect provider artifact {}: {error}",
                source_path.display()
            )
        })?;
        if link_metadata.file_type().is_symlink() {
            let target_metadata = fs::metadata(&source_path).map_err(|error| {
                format!(
                    "resolve provider artifact symlink {}: {error}",
                    source_path.display()
                )
            })?;
            if target_metadata.is_dir() {
                return Err(format!(
                    "provider workspace artifact contains directory symlink: {}",
                    source_path.display()
                ));
            }
            fs::copy(&source_path, &target_path)
                .map_err(|error| format!("copy provider artifact symlink target: {error}"))?;
            fs::set_permissions(&target_path, target_metadata.permissions())
                .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        } else if link_metadata.is_dir() {
            fs::create_dir(&target_path)
                .map_err(|error| format!("create provider artifact directory: {error}"))?;
            copy_artifact_directory(&source_path, &target_path)?;
        } else if link_metadata.is_file() {
            fs::copy(&source_path, &target_path)
                .map_err(|error| format!("copy provider artifact file: {error}"))?;
            fs::set_permissions(&target_path, link_metadata.permissions())
                .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        } else {
            return Err(format!(
                "provider workspace artifact contains unsupported entry: {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

fn launcher_script(
    artifact_root: &Path,
    artifact_entrypoint: &Path,
    launch: Option<&WorkspaceLaunchDescriptor>,
) -> Result<String, String> {
    let (program, args) = match launch {
        Some(launch) => {
            let program = if launch.program_relative_to_artifact {
                artifact_root
                    .join(&launch.program)
                    .to_string_lossy()
                    .into_owned()
            } else {
                launch.program.clone()
            };
            let args = launch
                .args
                .iter()
                .map(|argument| {
                    if launch.args_relative_to_artifact {
                        artifact_root.join(argument).to_string_lossy().into_owned()
                    } else {
                        argument.clone()
                    }
                })
                .collect();
            (program, args)
        }
        None => (
            artifact_entrypoint.to_string_lossy().into_owned(),
            Vec::new(),
        ),
    };
    if program.is_empty() {
        return Err("provider workspace launcher program is empty".to_string());
    }
    let mut script = format!("#!/bin/sh\nexec {}", shell_quote(&program));
    for argument in args {
        script.push(' ');
        script.push_str(&shell_quote(&argument));
    }
    script.push_str(" \"$@\"\n");
    Ok(script)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("inspect launcher {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("chmod launcher {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Err("provider workspace launchers require a native Windows adapter".to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider_workspace.rs"]
mod tests;
