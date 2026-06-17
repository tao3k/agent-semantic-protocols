//! Install command routing and rev-first language provider installer.

use agent_semantic_runtime::{
    ensure_project_provider_bin_dir, ensure_project_provider_lock_dir, project_runtime_state,
};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::hook_runtime::{run_codex_plugin_install_args, run_hook_runtime_args};

#[derive(Clone, Copy)]
struct ProviderReleaseSpec {
    language_id: &'static str,
    provider_id: &'static str,
    repo: &'static str,
    binary: &'static str,
    supported_targets: &'static [&'static str],
}

const PROVIDER_RELEASES: &[ProviderReleaseSpec] = &[
    ProviderReleaseSpec {
        language_id: "rust",
        provider_id: "rs-harness",
        repo: "tao3k/rust-lang-project-harness",
        binary: "rs-harness",
        supported_targets: &[
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ],
    },
    ProviderReleaseSpec {
        language_id: "typescript",
        provider_id: "ts-harness",
        repo: "tao3k/typescript-lang-project-harness",
        binary: "ts-harness",
        supported_targets: &["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
    },
    ProviderReleaseSpec {
        language_id: "python",
        provider_id: "py-harness",
        repo: "tao3k/python-lang-project-harness",
        binary: "py-harness",
        supported_targets: &["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
    },
    ProviderReleaseSpec {
        language_id: "julia",
        provider_id: "julia-lang-project-harness",
        repo: "JuliaCN/JuliaLangProjectHarness.jl",
        binary: "asp-julia-harness",
        supported_targets: &["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
    },
    ProviderReleaseSpec {
        language_id: "gerbil-scheme",
        provider_id: "gerbil-scheme-harness",
        repo: "tao3k/gerbil-scheme-language-project-harness",
        binary: "gerbil-scheme-harness",
        supported_targets: &["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
    },
];

#[derive(Default)]
struct InstallArgs {
    project_root: PathBuf,
    rev: Option<String>,
    target: Option<String>,
    archive: Option<PathBuf>,
    repo_override: Option<String>,
}

pub(crate) fn run_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("hook") => run_install_hook(&args[1..]),
        Some("plugin") => run_install_plugin(&args[1..]),
        Some("language") => run_install_provider(&args[1..]),
        Some("help" | "--help" | "-h") | None => Err(usage()),
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
    let spec = provider_release(language_id)
        .ok_or_else(|| format!("unsupported provider language `{language_id}`"))?;
    let install_args = parse_install_args(&args[1..])?;
    let rev = install_args
        .rev
        .as_deref()
        .ok_or_else(|| "asp install language requires --rev <git-tag-or-rev>".to_string())?;
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    validate_target(spec, &target)?;
    let repo = install_args.repo_override.as_deref().unwrap_or(spec.repo);
    let provider_bin_dir = ensure_project_provider_bin_dir(&install_args.project_root)?;
    let provider_lock_dir = ensure_project_provider_lock_dir(&install_args.project_root)?;
    let provider_package_dir = provider_lock_dir
        .join(spec.language_id)
        .join(path_segment(rev))
        .join(&target);
    let local_archive = install_args.archive.clone();
    let archive_path = match local_archive.clone() {
        Some(path) => path,
        None => download_release_archive(repo, spec, rev, &target, &install_args.project_root)?,
    };
    let expected_sha256 = checksum_for_archive(repo, spec, rev, &target, local_archive.as_deref())?;
    let actual_sha256 = sha256_file(&archive_path)?;
    if let Some(expected_sha256) = expected_sha256.as_deref()
        && expected_sha256 != actual_sha256
    {
        return Err(format!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual_sha256}",
            archive_path.display()
        ));
    }
    let installed = install_archive_binary(
        &archive_path,
        spec,
        &target,
        &provider_bin_dir,
        &provider_package_dir,
    )?;
    let lock_path = provider_lock_dir.join(format!("{}.lock.toml", spec.language_id));
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            language_id: spec.language_id,
            provider_id: spec.provider_id,
            repo,
            rev,
            target: &target,
            binary: spec.binary,
            installed_path: &installed,
            package_path: &provider_package_dir,
            sha256: &actual_sha256,
            source: archive_path.display().to_string(),
        },
    )?;
    let state = project_runtime_state(&install_args.project_root)?;
    println!(
        "[asp-install] provider={} language={} rev={} target={} binary={} installedPath={} lock={} runtimeBinDir={}",
        spec.provider_id,
        spec.language_id,
        rev,
        target,
        spec.binary,
        installed.display(),
        lock_path.display(),
        state.runtime_bin_dir.display(),
    );
    Ok(())
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
                index += 1;
                parsed.rev = Some(required_value(args, index, "--rev")?.to_string());
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
                index += 1;
                parsed.archive = Some(PathBuf::from(required_value(args, index, "--archive")?));
            }
            "--repo" => {
                index += 1;
                parsed.repo_override = Some(required_value(args, index, "--repo")?.to_string());
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

fn provider_release(language_id: &str) -> Option<ProviderReleaseSpec> {
    PROVIDER_RELEASES
        .iter()
        .copied()
        .find(|spec| spec.language_id == language_id)
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

fn validate_target(spec: ProviderReleaseSpec, target: &str) -> Result<(), String> {
    if spec
        .supported_targets
        .iter()
        .any(|supported| supported == &target)
    {
        return Ok(());
    }
    Err(format!(
        "unsupported target `{target}` for provider {}; supported targets: {}",
        spec.provider_id,
        spec.supported_targets.join(", ")
    ))
}

fn asset_name(spec: ProviderReleaseSpec, target: &str) -> String {
    format!("{}-{target}.tar.gz", spec.binary)
}

fn checksum_name(spec: ProviderReleaseSpec, target: &str) -> String {
    format!("{}.sha256", asset_name(spec, target))
}

fn release_asset_url(repo: &str, rev: &str, asset_name: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{rev}/{asset_name}")
}

fn download_release_archive(
    repo: &str,
    spec: ProviderReleaseSpec,
    rev: &str,
    target: &str,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let asset_name = asset_name(spec, target);
    let url = release_asset_url(repo, rev, &asset_name);
    let download_dir = ensure_project_provider_lock_dir(project_root)?.join("downloads");
    fs::create_dir_all(&download_dir)
        .map_err(|error| format!("failed to create {}: {error}", download_dir.display()))?;
    let archive_path = download_dir.join(&asset_name);
    run_curl(&url, &archive_path)?;
    Ok(archive_path)
}

fn checksum_for_archive(
    repo: &str,
    spec: ProviderReleaseSpec,
    rev: &str,
    target: &str,
    local_archive: Option<&Path>,
) -> Result<Option<String>, String> {
    if local_archive.is_some() {
        return Ok(None);
    }
    let url = release_asset_url(repo, rev, &checksum_name(spec, target));
    let checksum_path = env::temp_dir().join(format!(
        "asp-provider-install-{}-{target}.sha256",
        spec.language_id
    ));
    run_curl(&url, &checksum_path)?;
    let checksum = fs::read_to_string(&checksum_path)
        .map_err(|error| format!("failed to read {}: {error}", checksum_path.display()))?;
    Ok(parse_sha256_checksum(&checksum))
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
    spec: ProviderReleaseSpec,
    target: &str,
    provider_bin_dir: &Path,
    provider_package_dir: &Path,
) -> Result<PathBuf, String> {
    let target_path = provider_bin_dir.join(binary_file_name(spec.binary, target));
    let package_binary = install_archive_package(archive_path, spec, target, provider_package_dir)?;
    write_provider_launcher(&package_binary, &target_path, target)?;
    Ok(target_path)
}

fn install_archive_package(
    archive_path: &Path,
    spec: ProviderReleaseSpec,
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
        let extracted = find_binary_in_dir(&staging, spec.binary, target).ok_or_else(|| {
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
        let package_binary = provider_package_dir.join(binary_file_name(spec.binary, target));
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

fn write_provider_launcher(
    source: &Path,
    target: &Path,
    target_triple: &str,
) -> Result<(), String> {
    if target_triple.contains("windows") {
        return copy_executable(source, target);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let contents = format!("#!/bin/sh\nexec {} \"$@\"\n", shell_single_quote(source));
    let temp = target.with_extension("tmp");
    fs::write(&temp, contents.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", temp.display()))?;
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

fn shell_single_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
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
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> --rev <rev> [--target <target>] [--project <root>] [--repo <owner/repo>] [--archive <path>]".to_string()
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
