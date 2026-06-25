//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::{ensure_project_provider_lock_dir, project_runtime_state};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::hook_runtime::{run_codex_plugin_install_args, run_hook_runtime_args};
use super::install_provider_target::{home_dir, resolve_provider_binary_install_target};
use super::org_capture;

const PINNED_LANGUAGE_RELEASES_TOML: &str = include_str!("../../pinned-language-releases.toml");

#[derive(Clone, Debug)]
struct ProviderReleaseSpec {
    language_id: String,
    provider_id: String,
    repo: String,
    release_version: String,
    download_base_url: String,
    binary: String,
    supported_targets: Vec<String>,
}

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
    supported_targets: Vec<String>,
}

#[derive(Default)]
struct InstallArgs {
    project_root: PathBuf,
    target: Option<String>,
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
    let provider_lock_dir = ensure_project_provider_lock_dir(&install_args.project_root)?;
    let provider_package_dir = provider_lock_dir
        .join(&spec.language_id)
        .join(path_segment(rev))
        .join(&target);
    let asset_name = asset_name(&spec, &target);
    let archive_source = release_asset_url(&spec, &asset_name);
    let archive_path = download_release_archive(&spec, &target, &install_args.project_root)?;
    let expected_sha256 = checksum_for_archive(&spec, &target)?;
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

fn asset_name(spec: &ProviderReleaseSpec, target: &str) -> String {
    format!("{}-{target}.tar.gz", spec.binary)
}

fn checksum_name(spec: &ProviderReleaseSpec, target: &str) -> String {
    format!("{}.sha256", asset_name(spec, target))
}

fn release_asset_url(spec: &ProviderReleaseSpec, asset_name: &str) -> String {
    format!(
        "{}/{}",
        spec.download_base_url.trim_end_matches('/'),
        asset_name
    )
}

fn download_release_archive(
    spec: &ProviderReleaseSpec,
    target: &str,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let asset_name = asset_name(spec, target);
    let url = release_asset_url(spec, &asset_name);
    let download_dir = ensure_project_provider_lock_dir(project_root)?.join("downloads");
    fs::create_dir_all(&download_dir)
        .map_err(|error| format!("failed to create {}: {error}", download_dir.display()))?;
    let archive_path = download_dir.join(&asset_name);
    run_curl(&url, &archive_path)?;
    Ok(archive_path)
}

fn checksum_for_archive(spec: &ProviderReleaseSpec, target: &str) -> Result<String, String> {
    let url = release_asset_url(spec, &checksum_name(spec, target));
    let checksum_path = env::temp_dir().join(format!(
        "asp-provider-install-{}-{target}.sha256",
        spec.language_id
    ));
    run_curl(&url, &checksum_path)?;
    let checksum = fs::read_to_string(&checksum_path)
        .map_err(|error| format!("failed to read {}: {error}", checksum_path.display()))?;
    parse_sha256_checksum(&checksum)
        .ok_or_else(|| format!("checksum file {url} did not contain a sha256 digest"))
}

fn run_curl(url: &str, output: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(output)
        .arg(url)
        .status()
        .map_err(|error| format!("failed to run curl for {url}: {error}"))?;
    if !status.success() {
        return Err(format!("curl failed for {url} with status {status}"));
    }
    Ok(())
}

fn parse_sha256_checksum(content: &str) -> Option<String> {
    content
        .split_whitespace()
        .find(|part| {
            part.len() == 64 && part.chars().all(|character| character.is_ascii_hexdigit())
        })
        .map(|value| value.to_ascii_lowercase())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn install_archive_binary(
    archive_path: &Path,
    spec: &ProviderReleaseSpec,
    target: &str,
    provider_binary_path: &Path,
    provider_package_dir: &Path,
) -> Result<PathBuf, String> {
    let package_binary = install_archive_package(archive_path, spec, target, provider_package_dir)?;
    install_executable_entrypoint(&package_binary, provider_binary_path)?;
    Ok(provider_binary_path.to_path_buf())
}

fn install_archive_package(
    archive_path: &Path,
    spec: &ProviderReleaseSpec,
    target: &str,
    provider_package_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(parent) = provider_package_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    if is_tar_gz(archive_path) {
        let staging = temporary_package_dir(provider_package_dir)?;
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging)
            .map_err(|error| format!("failed to create {}: {error}", staging.display()))?;
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(&staging)
            .status()
            .map_err(|error| {
                format!("failed to run tar for {}: {error}", archive_path.display())
            })?;
        if !status.success() {
            return Err(format!(
                "tar failed for {} with status {status}",
                archive_path.display()
            ));
        }
        let extracted = find_binary_in_dir(&staging, &spec.binary, target).ok_or_else(|| {
            format!(
                "archive {} did not contain executable {}",
                archive_path.display(),
                spec.binary
            )
        })?;
        let binary_relative = extracted
            .strip_prefix(&staging)
            .map_err(|error| {
                format!(
                    "failed to resolve {} under {}: {error}",
                    extracted.display(),
                    staging.display()
                )
            })?
            .to_path_buf();
        let _ = fs::remove_dir_all(provider_package_dir);
        fs::rename(&staging, provider_package_dir).map_err(|error| {
            format!(
                "failed to move {} to {}: {error}",
                staging.display(),
                provider_package_dir.display()
            )
        })?;
        Ok(provider_package_dir.join(binary_relative))
    } else {
        let _ = fs::remove_dir_all(provider_package_dir);
        fs::create_dir_all(provider_package_dir).map_err(|error| {
            format!(
                "failed to create {}: {error}",
                provider_package_dir.display()
            )
        })?;
        let package_binary = provider_package_dir.join(binary_file_name(&spec.binary, target));
        copy_executable(archive_path, &package_binary)?;
        Ok(package_binary)
    }
}

fn is_tar_gz(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

fn temporary_package_dir(package_dir: &Path) -> Result<PathBuf, String> {
    let parent = package_dir.parent().ok_or_else(|| {
        format!(
            "provider package path has no parent: {}",
            package_dir.display()
        )
    })?;
    let package_name = package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system time before Unix epoch: {error}"))?
        .as_nanos();
    Ok(parent.join(format!(".{package_name}.tmp-{nonce}")))
}

fn find_binary_in_dir(dir: &Path, binary: &str, target: &str) -> Option<PathBuf> {
    let expected = binary_file_name(binary, target);
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_binary_in_dir(&path, binary, target) {
                return Some(found);
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some(expected.as_str()) {
            return Some(path);
        }
    }
    None
}

fn binary_file_name(binary: &str, target: &str) -> String {
    if target.contains("windows") && !binary.ends_with(".exe") {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn copy_executable(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temp = target.with_extension("tmp");
    fs::copy(source, &temp).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            temp.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&temp)
            .map_err(|error| format!("failed to stat {}: {error}", temp.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&temp, permissions)
            .map_err(|error| format!("failed to chmod {}: {error}", temp.display()))?;
    }
    fs::rename(&temp, target).map_err(|error| {
        format!(
            "failed to move {} to {}: {error}",
            temp.display(),
            target.display()
        )
    })
}

fn install_executable_entrypoint(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp = target.with_extension("tmp");
        let _ = fs::remove_file(&temp);
        symlink(source, &temp).map_err(|error| {
            format!(
                "failed to symlink {} to {}: {error}",
                source.display(),
                temp.display()
            )
        })?;
        fs::rename(&temp, target).map_err(|error| {
            format!(
                "failed to move {} to {}: {error}",
                temp.display(),
                target.display()
            )
        })?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        copy_executable(source, target)
    }
}

fn path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
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
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> [PROJECT_ROOT] [--target <target>] [--project <root>]\n       language provider releases are pinned by asp; install target priority: .agents/asp.toml [languages.<language>].bin, SEMANTIC_AGENT_BIN_DIR, $HOME/.local/bin, PATH".to_string()
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
