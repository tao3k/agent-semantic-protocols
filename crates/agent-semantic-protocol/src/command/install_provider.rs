//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::{ensure_project_provider_lock_dir, project_runtime_state};
use serde::Deserialize;
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
use super::install_provider_target::resolve_provider_binary_install_target;
use super::org_capture;

#[cfg(test)]
use super::install_provider_archive::{checksum_name, parse_sha256_checksum};

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
    #[serde(default)]
    sha256_by_target: BTreeMap<String, String>,
}

#[derive(Default)]
struct InstallArgs {
    project_root: PathBuf,
    target: Option<String>,
    reconcile_receipt: bool,
    record_installed_receipt: Option<PathBuf>,
}

pub(crate) fn run_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("binary") => run_install_binary(&args[1..]),
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

fn run_install_binary(args: &[String]) -> Result<(), String> {
    let mut target = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                if target.is_some() {
                    return Err(
                        "asp install binary accepts exactly one --target install root".to_string(),
                    );
                }
                target = Some(args.get(index + 1).ok_or_else(usage).map(PathBuf::from)?);
                index += 2;
            }
            "help" | "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => return Err(usage()),
        }
    }
    let target = target.ok_or_else(usage)?;
    let source = env::current_exe()
        .map_err(|error| format!("failed to resolve current ASP binary: {error}"))?;
    let project_root = env::current_dir()
        .map_err(|error| format!("failed to resolve current project root: {error}"))?;
    let runtime_state = agent_semantic_runtime::project_runtime_state(&project_root)?;
    let _reconciliation_guard = super::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(
        &runtime_state.protocol_home,
    )?;
    let artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let installed =
        super::protocol_binary::install_protocol_binary_target(&source, &target, &artifact_root)?;
    println!(
        "[asp-install-binary] binaryPath={} binaryInstall={} binaryArtifactDigest={} digestAlgorithm=blake3-256 binarySwitch=atomic",
        installed.path.display(),
        installed.status,
        installed.artifact_digest,
    );
    Ok(())
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
        return super::cli_help::print_install_plugin_help();
    }
    run_codex_plugin_install_args(args)
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
    if install_args.reconcile_receipt {
        return super::install_provider_reconcile::reconcile_provider_install_receipt(
            language_id,
            &project_root,
        );
    }
    let spec = provider_release(language_id)?;
    let rev = spec.release_version.as_str();
    validate_target(&spec, &target)?;
    if let Some(installed_path) = install_args.record_installed_receipt {
        let installed_path = if installed_path.is_absolute() {
            installed_path
        } else {
            invocation_root.join(installed_path)
        };
        let expected_binary = binary_file_name(&spec.binary, &target);
        if installed_path.file_name().and_then(|name| name.to_str())
            != Some(expected_binary.as_str())
        {
            return Err(format!(
                "installed provider binary mismatch for language {language_id}: expected {expected_binary}, got {}",
                installed_path.display()
            ));
        }
        let package_path = installed_path.parent().ok_or_else(|| {
            format!(
                "installed provider binary has no parent directory: {}",
                installed_path.display()
            )
        })?;
        let installed_entrypoint_digest =
            agent_semantic_content_identity::file_content_digest_v1(&installed_path)?;
        let installed_entrypoint_metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed_path)?;
        let installed_sha256 = sha256_file(&installed_path)?;
        let provider_lock_dir = ensure_project_provider_lock_dir(&install_args.project_root)?;
        let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
        write_provider_lock(
            &lock_path,
            &ProviderInstallLock {
                schema_id: "asp.provider-install-lock.v1",
                language_id: &spec.language_id,
                provider_id: &spec.provider_id,
                source_kind: "develop-root-justfile",
                repo: None,
                rev: None,
                target: &target,
                binary: &spec.binary,
                installed_path: &installed_path,
                package_path,
                sha256: &installed_sha256,
                source: project_root.display().to_string(),
                source_snapshot_root: None,
                source_snapshot_algorithm: None,
                source_leaf_count: None,
                provider_digest: None,
                build_recipe_digest: None,
                artifact_digest: None,
                artifact_leaf_count: None,
                artifact_entrypoint: None,
                artifact_entrypoint_sha256: None,
                installed_entrypoint_digest: Some(&installed_entrypoint_digest),
                installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
                launcher_digest: None,
            },
        )?;
        println!(
            "[asp-install] provider={} language={} installMode=record-installed-receipt sourceKind=develop-root-justfile target={} binary={} sha256={} installedPath={} lock={} switch=atomic",
            spec.provider_id,
            spec.language_id,
            target,
            spec.binary,
            installed_sha256,
            installed_path.display(),
            lock_path.display(),
        );
        return Ok(());
    }
    let provider_binary = binary_file_name(&spec.binary, &target);
    let install_target =
        resolve_provider_binary_install_target(&spec.language_id, &provider_binary)?;
    let provider_lock_dir = ensure_project_provider_lock_dir(&install_args.project_root)?;
    let provider_package_dir = provider_lock_dir
        .join(&spec.language_id)
        .join(path_segment(rev))
        .join(&target);
    let asset_name = asset_name(&spec, &target);
    let archive_source = release_asset_url(&spec, &asset_name);
    let archive_path = download_release_archive(&spec, &target, &install_args.project_root)?;
    let published_sha256 = checksum_for_archive(&spec, &target, &install_args.project_root)?;
    let pinned_sha256 = pinned_release_sha256(&spec, &target)?;
    if let Some(pinned_sha256) = pinned_sha256
        && pinned_sha256 != published_sha256
    {
        return Err(format!(
            "release checksum sidecar mismatch for provider {} target {target}: pinned {pinned_sha256}, published {published_sha256}",
            spec.provider_id
        ));
    }
    let checksum_authority = if pinned_sha256.is_some() {
        "pinned-release+sidecar"
    } else {
        "release-sidecar"
    };
    let expected_sha256 = pinned_sha256.unwrap_or(&published_sha256);
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
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed)?;
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
            installed_entrypoint_digest: Some(&installed_entrypoint_digest),
            installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
            launcher_digest: None,
        },
    )?;
    let org_state_sync = org_capture::run_org_state_sync(&project_root)?;
    println!(
        "[asp-install] provider={} language={} installMode=locked-release rev={} target={} binary={} sha256={} checksumAuthority={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} orgState={} orgStateSync={}",
        spec.provider_id,
        spec.language_id,
        rev,
        target,
        spec.binary,
        actual_sha256,
        checksum_authority,
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
            "--reconcile-receipt" => {
                parsed.reconcile_receipt = true;
            }
            "--record-installed-receipt" => {
                index += 1;
                parsed.record_installed_receipt = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--record-installed-receipt",
                )?));
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
    if parsed.reconcile_receipt && parsed.record_installed_receipt.is_some() {
        return Err("--reconcile-receipt conflicts with --record-installed-receipt".to_string());
    }
    Ok(parsed)
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
        sha256_by_target: entry.sha256_by_target,
    })
}

fn pinned_release_sha256<'a>(
    spec: &'a ProviderReleaseSpec,
    target: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = spec.sha256_by_target.get(target) else {
        if spec.sha256_by_target.is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "missing pinned release sha256 for provider {} target {target}",
            spec.provider_id
        ));
    };
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "invalid pinned release sha256 for provider {} target {target}",
            spec.provider_id
        ));
    }
    Ok(Some(value))
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
    installed_entrypoint_metadata_digest: &'a str,
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
    contents.push_str(&format!(
        "installedEntrypointMetadataDigest = \"{}\"\n",
        toml_escape(lock.installed_entrypoint_metadata_digest)
    ));
    if let Some(value) = lock.launcher_digest {
        contents.push_str(&format!("launcherDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    super::install_provider_reconcile::atomic_write_provider_lock(path, contents.as_bytes())
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn usage() -> String {
    "usage: asp install binary --target <path>\n       asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> [PROJECT_ROOT] [--target <target>] [--project <root>]\n       release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)\n       develop mode: use the repository Justfile recipes; the root Justfile owns provider builds and installation (installMode=develop-workspace)".to_string()
}

fn install_hook_usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]".to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider.rs"]
mod install_provider_tests;
