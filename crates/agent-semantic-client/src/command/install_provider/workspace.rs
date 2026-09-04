//! Registry-driven development provider workspace build and artifact publication.

use std::collections::BTreeMap;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_provider_protocol::ProviderWorkspaceInstallDescriptor;
use agent_semantic_provider_protocol::WorkspaceLaunchDescriptor;
use agent_semantic_provider_protocol::WorkspaceRuntimeDependencyDescriptor;
use agent_semantic_provider_transport::OutputMode;
use agent_semantic_provider_transport::ProviderProcessSpec;
use agent_semantic_provider_transport::ProviderProcessSupervisor;
use agent_semantic_provider_transport::StdinMode;
use agent_semantic_provider_transport::provider_process_limits_from_environment;

use super::workspace_receipt as receipt;
pub(super) use receipt::record_registered_provider_workspace_install;

/// Materialized provider workspace ready for atomic publication.
pub(super) struct BuiltProviderWorkspace {
    source_root: PathBuf,
    entrypoint: PathBuf,
    pub(super) provider_registration:
        agent_semantic_provider_protocol::ProviderRegistrationDocument,
    launch: Option<WorkspaceLaunchDescriptor>,
    runtime_dependencies: Vec<BuiltWorkspaceRuntimeDependency>,
    _build_guard: agent_semantic_runtime::provider_workspace_artifact::ProviderWorkspaceBuildGuard,
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
    registration: &super::super::provider_install_registry::ProviderInstallRegistration,
) -> Result<BuiltProviderWorkspace, String> {
    let dev_root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    let provider_source_root = dev_root
        .join(&registration.source_root)
        .canonicalize()
        .map_err(|error| format!("canonicalize provider sourceRoot: {error}"))?;
    ensure_within(&provider_source_root, &dev_root, "provider sourceRoot")?;
    let reference = registration.workspace_install.as_str();
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
    descriptor.validate()?;
    descriptor.validate_registration_identity(
        &registration.language_id,
        &registration.provider_id,
        &registration.binary,
    )?;

    let descriptor_parent = descriptor_path.parent().ok_or_else(|| {
        format!(
            "workspace install descriptor has no parent directory: {}",
            descriptor_path.display()
        )
    })?;
    agent_semantic_schema_manager::SchemaManager::new(&dev_root)
        .materialize(std::slice::from_ref(&descriptor.language_id))
        .await?;
    let schema_bundle_reference = Path::new(&descriptor.schema_bundle_receipt);
    if schema_bundle_reference.is_absolute()
        || schema_bundle_reference.as_os_str().is_empty()
        || schema_bundle_reference
            .components()
            .any(|component| matches!(component, Component::RootDir | Component::Prefix(_)))
    {
        return Err(format!(
            "schemaBundleReceipt must be relative to the workspace descriptor: {}",
            descriptor.schema_bundle_receipt
        ));
    }
    let schema_bundle_receipt_path = descriptor_parent
        .join(schema_bundle_reference)
        .canonicalize()
        .map_err(|error| {
            format!(
                "resolve schemaBundleReceipt {}: {error}",
                descriptor.schema_bundle_receipt
            )
        })?;
    ensure_within(
        &schema_bundle_receipt_path,
        &provider_source_root,
        "schema bundle receipt",
    )?;
    let schema_bundle_receipt =
        agent_semantic_schema_manager::verify_bundle_receipt(&schema_bundle_receipt_path).await?;
    if schema_bundle_receipt.language_id != descriptor.language_id {
        return Err(format!(
            "schema bundle receipt language drift: expected={} actual={}",
            descriptor.language_id, schema_bundle_receipt.language_id
        ));
    }
    let descriptor_schema_path = descriptor_parent
        .join(&descriptor.schema)
        .canonicalize()
        .map_err(|error| {
            format!(
                "resolve provider workspace schema {}: {error}",
                descriptor.schema
            )
        })?;
    ensure_within(
        &descriptor_schema_path,
        &provider_source_root,
        "provider workspace schema",
    )?;
    let receipted_schema_path = schema_bundle_receipt_path
        .parent()
        .expect("schema bundle receipt has a parent")
        .join(agent_semantic_provider_protocol::PROVIDER_WORKSPACE_INSTALL_SCHEMA_FILE)
        .canonicalize()
        .map_err(|error| format!("resolve receipted provider workspace schema: {error}"))?;
    if descriptor_schema_path != receipted_schema_path {
        return Err(format!(
            "provider workspace schema is outside the receipted bundle: schema={} receipt={}",
            descriptor_schema_path.display(),
            schema_bundle_receipt_path.display()
        ));
    }

    let provider_registration_reference = Path::new(&descriptor.provider_registration);
    validate_relative_path(
        provider_registration_reference,
        false,
        "providerRegistration",
    )?;
    let provider_registration_path = descriptor_parent
        .join(provider_registration_reference)
        .canonicalize()
        .map_err(|error| {
            format!(
                "failed to resolve providerRegistration {} relative to {}: {error}",
                descriptor.provider_registration,
                descriptor_parent.display()
            )
        })?;
    ensure_within(
        &provider_registration_path,
        &provider_source_root,
        "provider registration",
    )?;
    let provider_registration_value: serde_json::Value =
        serde_json::from_slice(&fs::read(&provider_registration_path).map_err(|error| {
            format!(
                "read provider registration {}: {error}",
                provider_registration_path.display()
            )
        })?)
        .map_err(|error| {
            format!(
                "decode provider registration {}: {error}",
                provider_registration_path.display()
            )
        })?;
    let provider_registration = agent_semantic_provider_protocol::ProviderRegistrationDocument {
        language_id: descriptor.language_id.clone(),
        provider_id: descriptor.provider_id.clone(),
        registration: provider_registration_value,
    };
    provider_registration.validate()?;
    provider_registration.compiled_routes()?;

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
    // One workspace builder owns the mutable provider output until the complete
    // tree has been copied into the immutable CAS and the stable launcher has
    // been atomically published. This is a cross-process lock; no provider
    // script may race another installer on the declared derived path.
    let build_guard =
        agent_semantic_runtime::provider_workspace_artifact::acquire_provider_workspace_build_guard(
            &artifact_path,
        )?;
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
            &materialization.remove_env,
            &materialization.remove_env_prefixes,
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
        &descriptor.workspace_build.remove_env,
        &descriptor.workspace_build.remove_env_prefixes,
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
        provider_registration,
        launch: descriptor.workspace_artifact.launch,
        runtime_dependencies,
        _build_guard: build_guard,
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
    registration: &super::super::provider_install_registry::ProviderInstallRegistration,
    dev_root: &Path,
    program: &str,
    args: &[String],
    cwd: PathBuf,
    environment: &BTreeMap<String, String>,
    remove_environment: &[String],
    remove_environment_prefixes: &[String],
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
            remove_env: remove_environment.iter().cloned().collect(),
            remove_env_prefixes: remove_environment_prefixes.iter().cloned().collect(),
            stdin: StdinMode::Inherit,
            stdout: OutputMode::Tee,
            stderr: OutputMode::Tee,
            // Workspace builds are owned by the Tokio request future. Dropping the
            // request cancels the supervised process tree; a fixed wall-clock timer
            // is not a lifecycle authority and must not decide build correctness.
            limits: limits.with_timeout(None),
        })
        .await
        .map_err(|error| {
            format!(
                "registered provider {stage} gate failed: language={} provider={} error={error}",
                registration.language_id, registration.provider_id
            )
        });
    supervisor.shutdown().await;
    let output = result?;
    if !output.status.success() {
        return Err(format!(
            "registered provider {stage} failed: language={} provider={} status={}",
            registration.language_id, registration.provider_id, output.status
        ));
    }
    Ok(())
}

pub(super) async fn publish_provider_workspace(
    protocol_home: &Path,
    stable_entry: &Path,
    binary_artifact_root: &Path,
    registration: &super::super::provider_install_registry::ProviderInstallRegistration,
    built: BuiltProviderWorkspace,
) -> Result<PublishedProviderWorkspace, String> {
    let publication_root = protocol_home
        .join("runtime/provider-artifacts")
        .join(&registration.provider_id)
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
    let checkout_root =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            protocol_home,
        )?
        .ok_or_else(|| "provider workspace publication requires Runtime dev mode".to_owned())?;
    let installed = super::super::protocol_binary::install_qualified_provider_staging_target(
        &launcher,
        stable_entry,
        binary_artifact_root,
        &binary_identity,
        checkout_root,
    )
    .await?;
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
#[path = "../../../tests/unit/install_provider_workspace.rs"]
mod tests;
