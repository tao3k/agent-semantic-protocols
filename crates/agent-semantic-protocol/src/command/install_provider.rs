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
use super::install_provider_target::{
    ProviderBinaryInstallTarget, home_dir, resolve_provider_binary_install_target,
};
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
    let spec = provider_release(language_id)?;
    let install_args = parse_install_args(&args[1..])?;
    let rev = spec.release_version.as_str();
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    validate_target(&spec, &target)?;
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let project_root = absolute_project_root(&invocation_root, &install_args.project_root);
    let provider_binary = binary_file_name(&spec.binary, &target);
    let install_target = resolve_provider_binary_install_target(
        &spec.language_id,
        &provider_binary,
        home_dir().as_deref(),
    )?;
    if install_args.from_workspace {
        return install_workspace_provider_binary(
            &spec,
            &project_root,
            &provider_binary,
            &install_target,
        );
    }
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
    let installed = install_archive_binary(
        &archive_path,
        &spec,
        &target,
        &install_target.path,
        &provider_package_dir,
    )?;
    let lock_path = provider_lock_dir.join(format!("{}.lock.toml", spec.language_id));
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            language_id: &spec.language_id,
            provider_id: &spec.provider_id,
            repo: &spec.repo,
            rev,
            target: &target,
            binary: &spec.binary,
            installed_path: &installed,
            package_path: &provider_package_dir,
            sha256: &actual_sha256,
            source: archive_source,
        },
    )?;
    let state = project_runtime_state(&project_root)?;
    let org_state_sync = org_capture::run_org_state_sync(&project_root)?;
    println!(
        "[asp-install] provider={} language={} rev={} target={} binary={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} orgState={} orgStateSync={}",
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

fn install_workspace_provider_binary(
    spec: &ProviderReleaseSpec,
    project_root: &Path,
    provider_binary: &str,
    install_target: &ProviderBinaryInstallTarget,
) -> Result<(), String> {
    let workspace_binary = project_root.join(".bin").join(provider_binary);
    if !workspace_binary.is_file() {
        return Err(format!(
            "workspace provider binary is missing at {}; build or refresh languages before running `asp install language {} --from-workspace`",
            workspace_binary.display(),
            spec.language_id
        ));
    }
    install_executable_entrypoint(&workspace_binary, &install_target.path)?;
    let state = project_runtime_state(project_root)?;
    let org_state_sync = org_capture::run_org_state_sync(project_root)?;
    println!(
        "[asp-install] provider={} language={} source=workspace-bin binary={} workspaceBin={} installedPath={} installTargetSource={} runtimeBinDir={} orgState={} orgStateSync={}",
        spec.provider_id,
        spec.language_id,
        spec.binary,
        workspace_binary.display(),
        install_target.path.display(),
        install_target.source,
        state.runtime_bin_dir.display(),
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
    let mut manifest: PinnedLanguageReleaseManifest = toml::from_str(PINNED_LANGUAGE_RELEASES_TOML)
        .map_err(|error| format!("failed to parse pinned language releases: {error}"))?;
    let Some(entry) = manifest.languages.remove(language_id) else {
        let supported = manifest
            .languages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unsupported provider language `{language_id}`; pinned languages: {supported}"
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
    language_id: &'a str,
    provider_id: &'a str,
    repo: &'a str,
    rev: &'a str,
    target: &'a str,
    binary: &'a str,
    installed_path: &'a Path,
    package_path: &'a Path,
    sha256: &'a str,
    source: String,
}

fn write_provider_lock(path: &Path, lock: &ProviderInstallLock<'_>) -> Result<(), String> {
    let contents = format!(
        "language = \"{}\"\nprovider = \"{}\"\nrepo = \"{}\"\nrev = \"{}\"\ntarget = \"{}\"\nbinary = \"{}\"\ninstalledPath = \"{}\"\npackagePath = \"{}\"\nsha256 = \"{}\"\nsource = \"{}\"\n",
        toml_escape(lock.language_id),
        toml_escape(lock.provider_id),
        toml_escape(lock.repo),
        toml_escape(lock.rev),
        toml_escape(lock.target),
        toml_escape(lock.binary),
        toml_escape(&lock.installed_path.display().to_string()),
        toml_escape(&lock.package_path.display().to_string()),
        toml_escape(lock.sha256),
        toml_escape(&lock.source),
    );
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
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> [PROJECT_ROOT] [--target <target>] [--project <root>] [--from-workspace]\n       language provider releases are pinned by asp by default; --from-workspace refreshes $HOME/.local/bin from <project>/.bin/<provider-binary>; install target priority: .agents/asp.toml [languages.<language>].bin, SEMANTIC_AGENT_BIN_DIR, $HOME/.local/bin, PATH".to_string()
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
