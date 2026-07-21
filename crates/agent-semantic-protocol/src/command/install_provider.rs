//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::{ensure_project_provider_lock_dir, project_runtime_state};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::hook_runtime::{run_codex_plugin_install_args, run_hook_runtime_args};
use super::install_provider_archive::{
    asset_name, binary_file_name, checksum_for_archive, download_release_archive,
    install_archive_binary, install_executable_entrypoint, path_segment, release_asset_url,
    sha256_file,
};
use super::install_provider_release::ProviderReleaseSpec;
use super::install_provider_target::{
    ProviderBinaryInstallTarget, home_dir, resolve_provider_binary_install_target,
};
use super::org_capture;

#[cfg(test)]
use super::install_provider_archive::{checksum_name, parse_sha256_checksum};

use super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot;
use super::install_provider_workspace_cas::install_workspace_artifact_from_cas;

const PINNED_LANGUAGE_RELEASES_TOML: &str = include_str!("../../pinned-language-releases.toml");

#[derive(Deserialize)]
struct PinnedLanguageReleaseManifest {
    languages: BTreeMap<String, PinnedLanguageReleaseEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinnedLanguageReleaseEntry {
    provider: String,
    repo: String,
    version: String,
    download_base_url: String,
    binary: String,
    archive_prefix: Option<String>,
    archive_binary: Option<String>,
    require_native_binary: Option<bool>,
    supported_targets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceBuildSpec {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    working_directory: String,
    derived_paths: Vec<String>,
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
}

#[derive(Default)]
struct InstallArgs {
    project_root: PathBuf,
    target: Option<String>,
    from_workspace: bool,
}

pub(crate) fn run_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("hook") => run_install_hook(&args[1..]),
        Some("plugin") => run_install_plugin(&args[1..]),
        Some("language") => run_install_provider(&args[1..]),
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        None => Err(usage()),
        Some(_) => Err(usage()),
    }
}

fn run_install_hook(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        println!("{}", install_hook_usage());
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--codex") {
        return Err(
            "Codex plugin installation uses `asp install plugin --codex [PROJECT_ROOT]`"
                .to_string(),
        );
    }

    let mut forwarded = vec!["install".to_string()];
    forwarded.extend(args.iter().cloned());
    run_hook_runtime_args(forwarded)
}

fn run_install_plugin(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        println!("{}", install_plugin_usage());
        return Ok(());
    }

    match args.first().map(String::as_str) {
        Some("--codex") => run_codex_plugin_install_args(&args[1..]),
        Some(target) => Err(format!(
            "unsupported plugin target: {target}; expected --codex"
        )),
        None => unreachable!("empty plugin installation args handled before dispatch"),
    }
}

fn has_help_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
}

fn run_install_provider(args: &[String]) -> Result<(), String> {
    let Some(language_id) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    if matches!(language_id, "help" | "--help" | "-h") {
        return Err(usage());
    }
    let install_args = parse_install_args(&args[1..])?;
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let project_root = absolute_project_root(&invocation_root, &install_args.project_root);
    if install_args.from_workspace {
        let descriptor = super::install_provider_workspace_descriptor::workspace_install_descriptor_for_language(
            language_id,
        )?;
        let provider_binary = binary_file_name(&descriptor.binary, &target);
        let install_target = resolve_provider_binary_install_target(
            language_id,
            &provider_binary,
            home_dir().as_deref(),
        )?;
        return install_workspace_provider_binary(
            &descriptor,
            language_id,
            &project_root,
            &target,
            &install_target,
        );
    }
    let spec = provider_release(language_id)?;
    let rev = spec.release_version.as_str();
    validate_target(&spec, &target)?;
    let provider_binary = binary_file_name(&spec.binary, &target);
    let install_target = resolve_provider_binary_install_target(
        &spec.language_id,
        &provider_binary,
        home_dir().as_deref(),
    )?;
    let provider_lock_dir = ensure_project_provider_lock_dir(&install_args.project_root)?;
    let provider_package_dir = provider_lock_dir
        .join(&spec.language_id)
        .join(path_segment(rev))
        .join(&target);
    let asset_name = asset_name(&spec, &target);
    let archive_source = release_asset_url(&spec, &asset_name);
    let archive_path = download_release_archive(&spec, &target, &install_args.project_root)?;
    let expected_sha256 = checksum_for_archive(&spec, &target, &install_args.project_root)?;
    let actual_sha256 = sha256_file(&archive_path)?;
    if expected_sha256 != actual_sha256 {
        return Err(format!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual_sha256}",
            archive_path.display()
        ));
    }
    let installed_entrypoint = install_archive_binary(
        &archive_path,
        &spec,
        &target,
        &install_target.path,
        &provider_package_dir,
    )?;
    let state = project_runtime_state(&project_root)?;
    let runtime_artifact = state.runtime_bin_dir.join(&spec.binary);
    if installed_entrypoint != runtime_artifact {
        install_executable_entrypoint(&installed_entrypoint, &runtime_artifact)?;
    }
    let installed = runtime_artifact;
    let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            language_id: &spec.language_id,
            provider_id: &spec.provider_id,
            source_kind: "release",
            repo: Some(&spec.repo),
            rev: Some(rev),
            target: &target,
            binary: &spec.binary,
            installed_path: &installed,
            package_path: &provider_package_dir,
            sha256: &actual_sha256,
            source: archive_source,
            source_snapshot_root: None,
            source_snapshot_algorithm: None,
            source_leaf_count: None,
            provider_digest: None,
            build_recipe_digest: None,
            artifact_digest: None,
            artifact_leaf_count: None,
            artifact_entrypoint: None,
            artifact_entrypoint_sha256: None,
            installed_entrypoint_digest: None,
            launcher_digest: None,
        },
    )?;
    let org_state_sync = org_capture::run_org_state_sync(&project_root)?;
    println!(
        "[asp-install] provider={} language={} installMode=locked-release rev={} target={} binary={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} orgState={} orgStateSync={}",
        spec.provider_id,
        spec.language_id,
        rev,
        target,
        spec.binary,
        installed.display(),
        install_target.source,
        lock_path.display(),
        state.runtime_bin_dir.display(),
        state.protocol_home.join("org").display(),
        org_state_sync.status,
    );
    Ok(())
}

fn absolute_project_root(invocation_root: &Path, project_root: &Path) -> PathBuf {
    if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        invocation_root.join(project_root)
    }
}

fn parse_install_args(args: &[String]) -> Result<InstallArgs, String> {
    let mut parsed = InstallArgs {
        project_root: PathBuf::from("."),
        ..InstallArgs::default()
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--rev" => {
                return Err(
                    "asp install language uses pinned provider releases; --rev is not supported"
                        .to_string(),
                );
            }
            "--target" => {
                index += 1;
                parsed.target = Some(required_value(args, index, "--target")?.to_string());
            }
            "--from-workspace" | "--local-dev" => {
                parsed.from_workspace = true;
            }
            "--project" | "--workspace" => {
                index += 1;
                parsed.project_root = PathBuf::from(required_value(args, index, "--project")?);
            }
            "--archive" => {
                return Err(
                    "asp install language uses pinned GitHub release downloads; --archive is not supported"
                        .to_string(),
                );
            }
            "--repo" => {
                return Err(
                    "asp install language uses pinned provider repositories; --repo is not supported"
                        .to_string(),
                );
            }
            "help" | "--help" | "-h" => return Err(usage()),
            flag if flag.starts_with('-') => {
                return Err(format!("unknown install option `{flag}`"));
            }
            path => parsed.project_root = PathBuf::from(path),
        }
        index += 1;
    }
    Ok(parsed)
}

#[derive(Debug)]
pub(super) struct WorkspaceBuildReceipt {
    pub(super) source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub(super) build_recipe_digest: String,
    pub(super) artifact_digest: String,
    pub(super) artifact_leaf_count: usize,
    pub(super) entrypoint_sha256: String,
}

#[derive(Debug)]
pub(super) struct MaterializedWorkspaceArtifact {
    pub(super) source_cas_root: PathBuf,
    pub(super) build_root: PathBuf,
    pub(super) workspace_root: PathBuf,
    pub(super) workspace_entrypoint: PathBuf,
    pub(super) entrypoint_relative: PathBuf,
    pub(super) launch:
        Option<super::install_provider_workspace_artifact::WorkspaceArtifactLaunchSpec>,
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
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "provider workspace build {field} must be a project-relative path without parent traversal: {relative}"
        ));
    }
    Ok(project_root.join(path))
}

fn workspace_build_recipe_digest(
    spec: &super::install_provider_workspace_descriptor::ProviderWorkspaceInstallDescriptor,
    build: &WorkspaceBuildSpec,
) -> Result<String, String> {
    let mut payload = serde_json::to_vec(build)
        .map_err(|error| format!("failed to encode workspace build recipe: {error}"))?;
    payload.push(0);
    payload.extend_from_slice(spec.binary.as_bytes());
    payload.push(0);
    let artifact = serde_json::to_vec(&spec.workspace_artifact)
        .map_err(|error| format!("failed to encode workspace artifact recipe: {error}"))?;
    payload.extend_from_slice(&artifact);
    Ok(agent_semantic_content_identity::hash_blob(&payload).value)
}

fn rendered_workspace_build_env(
    build: &WorkspaceBuildSpec,
    workspace_root: &Path,
) -> std::collections::BTreeMap<String, String> {
    let workspace_root = workspace_root.to_string_lossy();
    build
        .env
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                value.replace("${ASP_WORKSPACE_ROOT}", &workspace_root),
            )
        })
        .collect()
}

fn materialize_workspace_provider_binary(
    spec: &super::install_provider_workspace_descriptor::ProviderWorkspaceInstallDescriptor,
    project_root: &Path,
    state: &agent_semantic_runtime::ProjectRuntimeState,
) -> Result<(MaterializedWorkspaceArtifact, WorkspaceBuildReceipt), String> {
    let artifact = spec.workspace_artifact.clone();
    let live_workspace_artifact_root =
        resolve_workspace_relative_path(project_root, &artifact.root, "workspaceArtifact.root")?;
    let build = &spec.workspace_build;
    if build.program.trim().is_empty() {
        return Err(format!(
            "provider {} workspaceBuild program must not be empty",
            spec.provider_id
        ));
    }
    let program_name = Path::new(&build.program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(build.program.as_str());
    if matches!(program_name, "sh" | "bash" | "zsh" | "fish") {
        return Err(format!(
            "provider {} workspaceBuild must use an executable plus argv, not a shell interpreter",
            spec.provider_id
        ));
    }
    let live_working_directory = resolve_workspace_relative_path(
        project_root,
        &build.working_directory,
        "workingDirectory",
    )?;
    if !live_working_directory.is_dir() {
        return Err(format!(
            "provider {} workspaceBuild working directory is missing at {}",
            spec.provider_id,
            live_working_directory.display()
        ));
    }
    let derived_paths = build
        .derived_paths
        .iter()
        .map(|path| resolve_workspace_relative_path(project_root, path, "derivedPaths"))
        .collect::<Result<Vec<_>, _>>()?;
    if !derived_paths
        .iter()
        .any(|derived| live_workspace_artifact_root.starts_with(derived))
    {
        return Err(format!(
            "provider {} workspaceArtifact.root {} must be contained by one workspaceBuild.derivedPaths boundary",
            spec.provider_id,
            live_workspace_artifact_root.display()
        ));
    }
    let build_recipe_digest = workspace_build_recipe_digest(spec, build)?;
    let before =
        capture_workspace_build_snapshot(project_root, &derived_paths, &build_recipe_digest)?;
    let source_cas_root = materialize_workspace_source_cas(state, project_root, &before)?;
    let sandbox = materialize_workspace_build_sandbox(
        state,
        &spec.provider_id,
        &build_recipe_digest,
        &source_cas_root,
        &before,
    )?;
    let working_directory = resolve_workspace_relative_path(
        &sandbox.root,
        &build.working_directory,
        "workingDirectory",
    )?;
    fs::create_dir_all(&working_directory).map_err(|error| {
        format!(
            "failed to create pinned workspace build directory {}: {error}",
            working_directory.display()
        )
    })?;
    let workspace_artifact_root =
        resolve_workspace_relative_path(&sandbox.root, &artifact.root, "workspaceArtifact.root")?;
    let derived_paths = build
        .derived_paths
        .iter()
        .map(|path| resolve_workspace_relative_path(&sandbox.root, path, "derivedPaths"))
        .collect::<Result<Vec<_>, _>>()?;
    let configured_program = Path::new(&build.program);
    let build_program = if configured_program.is_absolute() {
        configured_program
            .strip_prefix(project_root)
            .map(|relative| sandbox.root.join(relative))
            .unwrap_or_else(|_| configured_program.to_path_buf())
    } else {
        configured_program.to_path_buf()
    };
    let mut command = std::process::Command::new(&build_program);
    command
        .args(&build.args)
        .current_dir(&working_directory)
        .env("ASP_WORKSPACE_ROOT", &sandbox.root)
        .envs(rendered_workspace_build_env(build, &sandbox.root));
    let status = command.status().map_err(|error| {
        format!(
            "failed to start workspace build for provider {} with program `{}`: {error}",
            spec.provider_id,
            build_program.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "workspace build failed for provider {} with status {status}",
            spec.provider_id
        ));
    }
    let after =
        capture_workspace_build_snapshot(&sandbox.root, &derived_paths, &build_recipe_digest)?;
    if before.evidence.root_digest != after.evidence.root_digest
        || before.evidence.leaf_count != after.evidence.leaf_count
    {
        let changed_paths = before.changed_paths(&after).join(",");
        return Err(format!(
            "pinned workspace source changed inside provider {} build sandbox: beforeRoot={} afterRoot={} changedPaths={changed_paths}",
            spec.provider_id, before.evidence.root_digest, after.evidence.root_digest
        ));
    }
    let workspace_artifact_metadata = fs::symlink_metadata(&workspace_artifact_root).map_err(|error| {
        format!(
            "workspace build for provider {} completed without producing configured artifact root {}: {error}",
            spec.provider_id,
            workspace_artifact_root.display()
        )
    })?;
    let entrypoint_relative = PathBuf::from(&artifact.entrypoint);
    if entrypoint_relative.is_absolute()
        || entrypoint_relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "provider {} workspaceArtifact.entrypoint must be artifact-relative without parent traversal: {}",
            spec.provider_id, artifact.entrypoint
        ));
    }
    let workspace_entrypoint = if workspace_artifact_metadata.file_type().is_file() {
        if artifact.entrypoint != "." {
            return Err(format!(
                "provider {} single-file workspaceArtifact.entrypoint must be `.`",
                spec.provider_id
            ));
        }
        workspace_artifact_root.clone()
    } else if workspace_artifact_metadata.file_type().is_dir() {
        workspace_artifact_root.join(&entrypoint_relative)
    } else {
        return Err(format!(
            "provider {} workspaceArtifact.root has unsupported type at {}",
            spec.provider_id,
            workspace_artifact_root.display()
        ));
    };
    if !workspace_entrypoint.is_file() {
        return Err(format!(
            "workspace build for provider {} completed without producing configured entrypoint {}",
            spec.provider_id,
            workspace_entrypoint.display()
        ));
    }
    let artifact_snapshot = capture_workspace_artifact_snapshot(&workspace_artifact_root)?;
    let entrypoint_sha256 = sha256_file(&workspace_entrypoint)?;
    let build_root = sandbox.persist();
    Ok((
        MaterializedWorkspaceArtifact {
            source_cas_root,
            build_root,
            workspace_root: workspace_artifact_root,
            workspace_entrypoint,
            entrypoint_relative,
            launch: artifact.launch,
        },
        WorkspaceBuildReceipt {
            source_snapshot: before.evidence,
            build_recipe_digest,
            artifact_digest: artifact_snapshot.root_digest,
            artifact_leaf_count: artifact_snapshot.leaf_count,
            entrypoint_sha256,
        },
    ))
}

fn install_workspace_provider_binary(
    spec: &super::install_provider_workspace_descriptor::ProviderWorkspaceInstallDescriptor,
    language_id: &str,
    project_root: &Path,
    target: &str,
    install_target: &ProviderBinaryInstallTarget,
) -> Result<(), String> {
    let state = project_runtime_state(project_root)?;
    let (workspace_artifact, build_receipt) =
        materialize_workspace_provider_binary(spec, project_root, &state)?;
    let runtime_artifact = state.runtime_bin_dir.join(&spec.binary);
    let installed = install_workspace_artifact_from_cas(
        spec,
        &state,
        &workspace_artifact,
        &build_receipt,
        &runtime_artifact,
        &install_target.path,
    );
    let cleanup = remove_workspace_snapshot_tree(&workspace_artifact.build_root);
    let installed = installed?;
    cleanup?;
    let provider_lock_dir = ensure_project_provider_lock_dir(project_root)?;
    let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            language_id,
            provider_id: &spec.provider_id,
            source_kind: "workspace",
            repo: None,
            rev: None,
            target,
            binary: &spec.binary,
            installed_path: &runtime_artifact,
            package_path: &installed.cas_root,
            sha256: &installed.installed_sha256,
            source: format!(
                "workspace+blake3://{}",
                build_receipt.source_snapshot.root_digest
            ),
            source_snapshot_root: Some(&build_receipt.source_snapshot.root_digest),
            source_snapshot_algorithm: Some(&build_receipt.source_snapshot.algorithm),
            source_leaf_count: Some(build_receipt.source_snapshot.leaf_count),
            provider_digest: Some(&build_receipt.source_snapshot.provider_digest),
            build_recipe_digest: Some(&build_receipt.build_recipe_digest),
            artifact_digest: Some(&build_receipt.artifact_digest),
            artifact_leaf_count: Some(build_receipt.artifact_leaf_count),
            artifact_entrypoint: Some(&workspace_artifact.entrypoint_relative),
            artifact_entrypoint_sha256: Some(&build_receipt.entrypoint_sha256),
            installed_entrypoint_digest: Some(&installed.installed_digest),
            launcher_digest: installed.launcher_digest.as_deref(),
        },
    )?;
    let org_state_sync = org_capture::run_org_state_sync(project_root)?;
    println!(
        "[asp-install] provider={} language={} installMode=develop-workspace source=workspace-build binary={} workspaceSourceCAS={} workspaceArtifact={} workspaceEntrypoint={} immutableArtifact={} immutableEntrypoint={} installedPath={} runtimeArtifact={} installTargetSource={} runtimeBinDir={} sourceSnapshotRoot={} sourceSnapshotAlgorithm={} sourceLeafCount={} providerDigest={} buildRecipeDigest={} artifactDigest={} artifactLeafCount={} artifactEntrypointSha256={} installedEntrypointDigest={} sha256={} launcherDigest={} lock={} orgState={} orgStateSync={}",
        spec.provider_id,
        language_id,
        spec.binary,
        workspace_artifact.source_cas_root.display(),
        workspace_artifact.workspace_root.display(),
        workspace_artifact.workspace_entrypoint.display(),
        installed.cas_root.display(),
        installed.cas_entrypoint.display(),
        install_target.path.display(),
        runtime_artifact.display(),
        install_target.source,
        state.runtime_bin_dir.display(),
        build_receipt.source_snapshot.root_digest,
        build_receipt.source_snapshot.algorithm,
        build_receipt.source_snapshot.leaf_count,
        build_receipt.source_snapshot.provider_digest,
        build_receipt.build_recipe_digest,
        build_receipt.artifact_digest,
        build_receipt.artifact_leaf_count,
        build_receipt.entrypoint_sha256,
        installed.installed_digest,
        installed.installed_sha256,
        installed.launcher_digest.as_deref().unwrap_or("none"),
        lock_path.display(),
        state.protocol_home.join("org").display(),
        org_state_sync.status,
    );
    Ok(())
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.starts_with('-') && !value.is_empty())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn provider_release(language_id: &str) -> Result<ProviderReleaseSpec, String> {
    let mut manifest = pinned_language_release_manifest()?;
    let Some(entry) = manifest.languages.remove(language_id) else {
        let supported = manifest
            .languages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "[asp-install-error] state=locked-release-unavailable installMode=locked-release language={language_id} reason=language-not-pinned pinnedLanguages={supported}"
        ));
    };
    Ok(ProviderReleaseSpec {
        language_id: language_id.to_string(),
        provider_id: entry.provider,
        repo: entry.repo,
        release_version: entry.version,
        download_base_url: entry.download_base_url,
        archive_prefix: entry.archive_prefix.unwrap_or_else(|| entry.binary.clone()),
        archive_binary: entry.archive_binary.unwrap_or_else(|| entry.binary.clone()),
        require_native_binary: entry.require_native_binary.unwrap_or(false),
        binary: entry.binary,
        supported_targets: entry.supported_targets,
    })
}

pub(super) fn has_pinned_language_release(language_id: &str) -> Result<bool, String> {
    Ok(pinned_language_release_manifest()?
        .languages
        .contains_key(language_id))
}

fn pinned_language_release_manifest() -> Result<PinnedLanguageReleaseManifest, String> {
    toml::from_str(PINNED_LANGUAGE_RELEASES_TOML)
        .map_err(|error| format!("failed to parse pinned language releases: {error}"))
}

fn host_target_triple() -> Option<String> {
    let arch = env::consts::ARCH;
    let os = env::consts::OS;
    match (arch, os) {
        ("aarch64", "macos") => Some("aarch64-apple-darwin".to_string()),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu".to_string()),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu".to_string()),
        ("x86_64", "windows") => Some("x86_64-pc-windows-msvc".to_string()),
        _ => None,
    }
}

fn validate_target(spec: &ProviderReleaseSpec, target: &str) -> Result<(), String> {
    if spec
        .supported_targets
        .iter()
        .any(|supported| supported == target)
    {
        return Ok(());
    }
    Err(format!(
        "unsupported target `{target}` for provider {}; supported targets: {}",
        spec.provider_id,
        spec.supported_targets.join(", ")
    ))
}

struct ProviderInstallLock<'a> {
    schema_id: &'a str,
    language_id: &'a str,
    provider_id: &'a str,
    source_kind: &'a str,
    repo: Option<&'a str>,
    rev: Option<&'a str>,
    target: &'a str,
    binary: &'a str,
    installed_path: &'a Path,
    package_path: &'a Path,
    sha256: &'a str,
    source: String,
    source_snapshot_root: Option<&'a str>,
    source_snapshot_algorithm: Option<&'a str>,
    source_leaf_count: Option<usize>,
    provider_digest: Option<&'a str>,
    build_recipe_digest: Option<&'a str>,
    artifact_digest: Option<&'a str>,
    artifact_leaf_count: Option<usize>,
    artifact_entrypoint: Option<&'a Path>,
    artifact_entrypoint_sha256: Option<&'a str>,
    installed_entrypoint_digest: Option<&'a str>,
    launcher_digest: Option<&'a str>,
}

fn write_provider_lock(path: &Path, lock: &ProviderInstallLock<'_>) -> Result<(), String> {
    let mut contents = format!(
        "schemaId = \"{}\"\nlanguage = \"{}\"\nprovider = \"{}\"\nsourceKind = \"{}\"\n",
        toml_escape(lock.schema_id),
        toml_escape(lock.language_id),
        toml_escape(lock.provider_id),
        toml_escape(lock.source_kind),
    );
    if let Some(repo) = lock.repo {
        contents.push_str(&format!("repo = \"{}\"\n", toml_escape(repo)));
    }
    if let Some(rev) = lock.rev {
        contents.push_str(&format!("rev = \"{}\"\n", toml_escape(rev)));
    }
    contents.push_str(&format!(
        "target = \"{}\"\nbinary = \"{}\"\ninstalledPath = \"{}\"\npackagePath = \"{}\"\nsha256 = \"{}\"\nsource = \"{}\"\n",
        toml_escape(lock.target),
        toml_escape(lock.binary),
        toml_escape(&lock.installed_path.display().to_string()),
        toml_escape(&lock.package_path.display().to_string()),
        toml_escape(lock.sha256),
        toml_escape(&lock.source),
    ));
    if let Some(value) = lock.source_snapshot_root {
        contents.push_str(&format!(
            "sourceSnapshotRoot = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.source_snapshot_algorithm {
        contents.push_str(&format!(
            "sourceSnapshotAlgorithm = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.source_leaf_count {
        contents.push_str(&format!("sourceLeafCount = {value}\n"));
    }
    if let Some(value) = lock.provider_digest {
        contents.push_str(&format!("providerDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.build_recipe_digest {
        contents.push_str(&format!("buildRecipeDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.artifact_digest {
        contents.push_str(&format!("artifactDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.artifact_leaf_count {
        contents.push_str(&format!("artifactLeafCount = {value}\n"));
    }
    if let Some(value) = lock.artifact_entrypoint {
        contents.push_str(&format!(
            "artifactEntrypoint = \"{}\"\n",
            toml_escape(&value.display().to_string())
        ));
    }
    if let Some(value) = lock.artifact_entrypoint_sha256 {
        contents.push_str(&format!(
            "artifactEntrypointSha256 = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.installed_entrypoint_digest {
        contents.push_str(&format!(
            "installedEntrypointDigest = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.launcher_digest {
        contents.push_str(&format!("launcherDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> [PROJECT_ROOT] [--target <target>] [--project <root>]\n       release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)\n       develop mode: use the repository Justfile recipes; they invoke the internal workspace mechanism (installMode=develop-workspace)".to_string()
}

fn install_hook_usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]".to_string()
}

fn install_plugin_usage() -> String {
    "usage: asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]".to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider.rs"]
mod install_provider_tests;
use super::install_provider_workspace_source::{
    capture_workspace_build_snapshot, materialize_workspace_build_sandbox,
    materialize_workspace_source_cas, remove_workspace_snapshot_tree,
};
