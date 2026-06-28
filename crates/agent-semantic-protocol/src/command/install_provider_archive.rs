//! Archive download and executable installation helpers for pinned providers.

use agent_semantic_runtime::ensure_project_provider_lock_dir;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::install_provider_release::ProviderReleaseSpec;

pub(super) fn asset_name(spec: &ProviderReleaseSpec, target: &str) -> String {
    format!("{}-{target}.tar.gz", spec.archive_prefix)
}

pub(super) fn checksum_name(spec: &ProviderReleaseSpec, target: &str) -> String {
    format!("{}.sha256", asset_name(spec, target))
}

pub(super) fn release_asset_url(spec: &ProviderReleaseSpec, asset_name: &str) -> String {
    format!(
        "{}/{}",
        spec.download_base_url.trim_end_matches('/'),
        asset_name
    )
}

pub(super) fn download_release_archive(
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

pub(super) fn checksum_for_archive(
    spec: &ProviderReleaseSpec,
    target: &str,
    project_root: &Path,
) -> Result<String, String> {
    let url = release_asset_url(spec, &checksum_name(spec, target));
    let download_dir = ensure_project_provider_lock_dir(project_root)?.join("downloads");
    fs::create_dir_all(&download_dir)
        .map_err(|error| format!("failed to create {}: {error}", download_dir.display()))?;
    let checksum_path = download_dir.join(checksum_name(spec, target));
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

pub(super) fn parse_sha256_checksum(content: &str) -> Option<String> {
    content
        .split_whitespace()
        .find(|part| {
            part.len() == 64 && part.chars().all(|character| character.is_ascii_hexdigit())
        })
        .map(|value| value.to_ascii_lowercase())
}

pub(super) fn sha256_file(path: &Path) -> Result<String, String> {
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

pub(super) fn install_archive_binary(
    archive_path: &Path,
    spec: &ProviderReleaseSpec,
    target: &str,
    provider_binary_path: &Path,
    provider_package_dir: &Path,
) -> Result<PathBuf, String> {
    let package_binary = install_archive_package(archive_path, spec, target, provider_package_dir)?;
    if spec.require_native_binary {
        validate_native_binary(&package_binary)?;
    }
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
        let extracted =
            find_binary_in_dir(&staging, &spec.archive_binary, target).ok_or_else(|| {
                format!(
                    "archive {} did not contain executable {}",
                    archive_path.display(),
                    spec.archive_binary
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
        let package_binary =
            provider_package_dir.join(binary_file_name(&spec.archive_binary, target));
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

pub(super) fn binary_file_name(binary: &str, target: &str) -> String {
    if target.contains("windows") && !binary.ends_with(".exe") {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn validate_native_binary(path: &Path) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut magic = [0_u8; 4];
    let read = file
        .read(&mut magic)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if read >= 4 && is_native_binary_magic(&magic) {
        return Ok(());
    }
    if read >= 2 && magic[..2] == *b"MZ" {
        return Ok(());
    }
    Err(format!(
        "provider archive binary {} is not a native executable; rebuild the release with --binary --release -O",
        path.display()
    ))
}

fn is_native_binary_magic(magic: &[u8; 4]) -> bool {
    matches!(
        magic,
        b"\x7FELF"
            | b"\xCF\xFA\xED\xFE"
            | b"\xFE\xED\xFA\xCF"
            | b"\xCE\xFA\xED\xFE"
            | b"\xFE\xED\xFA\xCE"
            | b"\xCA\xFE\xBA\xBE"
            | b"\xBE\xBA\xFE\xCA"
    )
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

pub(super) fn install_executable_entrypoint(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    copy_executable(source, target)
}

pub(super) fn path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}
