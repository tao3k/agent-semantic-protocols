// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Filesystem artifact identity derived from canonical metadata and content bytes.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Canonical filesystem metadata used to bind a file artifact observation.
pub struct FileArtifactMetadataV1 {
    pub canonical_path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix_nanos: u64,
    pub change_time_unix_nanos: Option<i64>,
}

/// Hashes canonical regular-file bytes with the V1 content digest algorithm.
pub fn file_content_digest_v1(path: &Path) -> Result<String, String> {
    let path = canonical_regular_file(path)?;
    let mut file = fs::File::open(&path)
        .map_err(|error| format!("failed to open artifact {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read artifact {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Reads canonical metadata for a regular file without identity aliases.
pub fn file_artifact_metadata_v1(path: &Path) -> Result<FileArtifactMetadataV1, String> {
    let canonical_path = canonical_regular_file(path)?;
    let metadata = fs::metadata(&canonical_path)
        .map_err(|error| format!("failed to inspect artifact {}: {error}", path.display()))?;
    Ok(FileArtifactMetadataV1 {
        canonical_path,
        size_bytes: metadata.len(),
        modified_unix_nanos: modified_unix_nanos(&metadata)?,
        change_time_unix_nanos: change_time_unix_nanos(&metadata),
    })
}

/// Hashes canonical metadata independently from file contents.
pub fn file_artifact_metadata_digest_v1(path: &Path) -> Result<String, String> {
    let metadata = file_artifact_metadata_v1(path)?;
    let canonical = format!(
        "asp.active-artifact-metadata.v1\0{}\0{}\0{}",
        metadata.size_bytes,
        metadata.modified_unix_nanos,
        metadata
            .change_time_unix_nanos
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
    );
    Ok(blake3::hash(canonical.as_bytes()).to_hex().to_string())
}

fn canonical_regular_file(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve artifact {}: {error}", path.display()))?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        format!(
            "failed to inspect artifact {}: {error}",
            canonical.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "artifact is not a regular file: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn modified_unix_nanos(metadata: &fs::Metadata) -> Result<u64, String> {
    let nanos = metadata
        .modified()
        .map_err(|error| format!("failed to read artifact modification time: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("artifact modification time predates UNIX epoch: {error}"))?
        .as_nanos();
    u64::try_from(nanos).map_err(|_| "artifact modification time exceeds u64".to_string())
}

#[cfg(unix)]
fn change_time_unix_nanos(metadata: &fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    metadata
        .ctime()
        .checked_mul(1_000_000_000)?
        .checked_add(metadata.ctime_nsec())
}

#[cfg(not(unix))]
fn change_time_unix_nanos(_metadata: &fs::Metadata) -> Option<i64> {
    None
}

#[cfg(test)]
#[path = "../tests/unit/file_artifact.rs"]
mod tests;
