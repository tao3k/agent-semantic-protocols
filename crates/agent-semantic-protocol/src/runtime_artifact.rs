use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

const GRAPH_TURBO_BUNDLE_NAME: &str = "asp-graph-turbo-resident.bundle";
const GRAPH_TURBO_EXECUTABLE_NAME: &str = "asp-graph-turbo-resident";
const GRAPH_TURBO_CONFIG_NAME: &str = "graph-turbo-resident-config.v1.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphTurboResidentArtifactReceipt {
    pub(crate) publication: &'static str,
    pub(crate) runtime_artifact_digest: String,
    pub(crate) locator: PathBuf,
    pub(crate) execution_artifact_digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphTurboResidentConfig<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    artifact_kind: &'static str,
    execution_artifact_locator: &'a Path,
    execution_artifact_digest: &'a str,
    runtime_artifact_digest: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredGraphTurboResidentConfig {
    schema_id: String,
    schema_version: String,
    artifact_kind: String,
    execution_artifact_locator: PathBuf,
    execution_artifact_digest: String,
    runtime_artifact_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedGraphTurboResidentConfig {
    pub(crate) locator: PathBuf,
    pub(crate) execution_artifact_digest: String,
    pub(crate) runtime_artifact_digest: String,
}

#[derive(Debug)]
struct BundleFile {
    source: PathBuf,
    relative: String,
    mode: u32,
    digest: String,
}

pub(crate) async fn publish_graph_turbo_resident_sibling(
    protocol_home: &Path,
    asp_executable: &Path,
) -> Result<GraphTurboResidentArtifactReceipt, String> {
    let release_root = asp_executable.parent().ok_or_else(|| {
        format!(
            "ASP release artifact has no sibling directory: {}",
            asp_executable.display()
        )
    })?;
    let source_bundle = release_root.join(GRAPH_TURBO_BUNDLE_NAME);
    let source_entry = source_bundle.join(format!(
        "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
        std::env::consts::EXE_SUFFIX
    ));
    require_standalone_bundle(&source_bundle, &source_entry).await?;

    let (artifact_digest, files) = standalone_bundle_digest(&source_bundle).await?;
    let execution_artifact_digest = file_digest(&source_entry).await?;
    let digest_hex = artifact_digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| "Graph Turbo bundle digest omitted blake3-256 prefix".to_owned())?;
    let artifact_parent = protocol_home
        .join("runtime/artifacts/blake3-256")
        .join(digest_hex);
    let artifact_path = artifact_parent.join(GRAPH_TURBO_BUNDLE_NAME);

    tokio::fs::create_dir_all(&artifact_parent)
        .await
        .map_err(|error| format!("failed to create Graph Turbo artifact parent: {error}"))?;
    let publication = if tokio::fs::try_exists(&artifact_path)
        .await
        .map_err(|error| format!("failed to inspect Graph Turbo artifact: {error}"))?
    {
        let (installed_digest, _) = standalone_bundle_digest(&artifact_path).await?;
        if installed_digest != artifact_digest {
            return Err(format!(
                "Graph Turbo content-addressed artifact drift: expected={artifact_digest} actual={installed_digest} path={}",
                artifact_path.display()
            ));
        }
        "current"
    } else {
        publish_bundle_atomically(&artifact_path, &files).await?;
        let (installed_digest, _) = standalone_bundle_digest(&artifact_path).await?;
        if installed_digest != artifact_digest {
            return Err(format!(
                "Graph Turbo artifact verification failed after publication: expected={artifact_digest} actual={installed_digest}"
            ));
        }
        "published"
    };

    let execution_artifact_locator = artifact_path.join(format!(
        "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
        std::env::consts::EXE_SUFFIX
    ));
    let installed_execution_digest = file_digest(&execution_artifact_locator).await?;
    if installed_execution_digest != execution_artifact_digest {
        return Err(format!(
            "Graph Turbo execution artifact drift after publication: expected={execution_artifact_digest} actual={installed_execution_digest}"
        ));
    }
    publish_config_atomically(
        protocol_home,
        &execution_artifact_locator,
        &execution_artifact_digest,
        &artifact_digest,
    )
    .await?;

    Ok(GraphTurboResidentArtifactReceipt {
        publication,
        runtime_artifact_digest: artifact_digest,
        locator: execution_artifact_locator,
        execution_artifact_digest,
    })
}

pub(crate) async fn validated_graph_turbo_resident_config(
    state_home: &Path,
) -> Result<Option<ValidatedGraphTurboResidentConfig>, String> {
    let config_path = state_home
        .join("runtime/server")
        .join(GRAPH_TURBO_CONFIG_NAME);
    let bytes = match tokio::fs::read(&config_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read Graph Turbo resident config: {error}")),
    };
    let config: StoredGraphTurboResidentConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode Graph Turbo resident config: {error}"))?;
    if config.schema_id != "agent.semantic-protocols.semantic-graph-turbo-resident-config"
        || config.schema_version != "1"
    {
        return Err("Graph Turbo resident config protocol identity drift".to_owned());
    }
    if config.artifact_kind != "standalone-directory" {
        return Err(format!(
            "Graph Turbo resident artifact kind is unsupported: {}",
            config.artifact_kind
        ));
    }
    let bundle_hex = validated_blake3_digest(
        "Graph Turbo runtime artifact",
        &config.runtime_artifact_digest,
    )?;
    validated_blake3_digest(
        "Graph Turbo execution artifact",
        &config.execution_artifact_digest,
    )?;
    let expected_bundle = state_home
        .join("runtime/artifacts/blake3-256")
        .join(bundle_hex)
        .join(GRAPH_TURBO_BUNDLE_NAME);
    let canonical_bundle = tokio::fs::canonicalize(&expected_bundle)
        .await
        .map_err(|error| {
            format!(
                "Graph Turbo content-addressed bundle is unavailable: path={} error={error}",
                expected_bundle.display()
            )
        })?;
    let canonical_locator = tokio::fs::canonicalize(&config.execution_artifact_locator)
        .await
        .map_err(|error| {
            format!(
                "Graph Turbo execution artifact is unavailable: path={} error={error}",
                config.execution_artifact_locator.display()
            )
        })?;
    let expected_locator = canonical_bundle.join(format!(
        "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
        std::env::consts::EXE_SUFFIX
    ));
    if canonical_locator != expected_locator {
        return Err(format!(
            "Graph Turbo execution locator escaped the admitted standalone bundle: expected={} actual={}",
            expected_locator.display(),
            canonical_locator.display()
        ));
    }
    let actual_execution_digest = file_digest(&canonical_locator).await?;
    if actual_execution_digest != config.execution_artifact_digest {
        return Err(format!(
            "Graph Turbo execution artifact digest drift: expected={} actual={} artifact={}",
            config.execution_artifact_digest,
            actual_execution_digest,
            canonical_locator.display()
        ));
    }
    Ok(Some(ValidatedGraphTurboResidentConfig {
        locator: canonical_locator,
        execution_artifact_digest: config.execution_artifact_digest,
        runtime_artifact_digest: config.runtime_artifact_digest,
    }))
}

fn validated_blake3_digest<'a>(label: &str, digest: &'a str) -> Result<&'a str, String> {
    let hex = digest
        .strip_prefix("blake3-256:")
        .filter(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| format!("{label} digest is invalid: {digest}"))?;
    Ok(hex)
}

async fn require_standalone_bundle(bundle: &Path, entry: &Path) -> Result<(), String> {
    let bundle_metadata = tokio::fs::symlink_metadata(bundle).await.map_err(|error| {
        format!(
            "Graph Turbo standalone sibling is unavailable: path={} error={error}",
            bundle.display()
        )
    })?;
    if !bundle_metadata.is_dir() || bundle_metadata.file_type().is_symlink() {
        return Err(format!(
            "Graph Turbo standalone sibling must be a real directory: {}",
            bundle.display()
        ));
    }
    let entry_metadata = tokio::fs::symlink_metadata(entry).await.map_err(|error| {
        format!(
            "Graph Turbo standalone entry executable is unavailable: path={} error={error}",
            entry.display()
        )
    })?;
    if !entry_metadata.is_file() || entry_metadata.file_type().is_symlink() {
        return Err(format!(
            "Graph Turbo standalone entry must be a real file: {}",
            entry.display()
        ));
    }
    Ok(())
}

async fn standalone_bundle_digest(bundle: &Path) -> Result<(String, Vec<BundleFile>), String> {
    let mut pending = vec![bundle.to_path_buf()];
    let mut paths = BTreeMap::new();
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(&directory).await.map_err(|error| {
            format!(
                "failed to enumerate Graph Turbo bundle directory {}: {error}",
                directory.display()
            )
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            format!(
                "failed to read Graph Turbo bundle directory {}: {error}",
                directory.display()
            )
        })? {
            let path = entry.path();
            let metadata = tokio::fs::symlink_metadata(&path)
                .await
                .map_err(|error| format!("failed to inspect Graph Turbo bundle entry: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "Graph Turbo standalone bundle forbids symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(format!(
                    "Graph Turbo standalone bundle contains unsupported entry: {}",
                    path.display()
                ));
            }
            let relative = path
                .strip_prefix(bundle)
                .map_err(|_| "Graph Turbo bundle entry escaped its root".to_owned())?
                .to_str()
                .ok_or_else(|| "Graph Turbo bundle entry is not UTF-8".to_owned())?
                .replace('\\', "/");
            let digest = file_digest(&path).await?;
            paths.insert(
                relative.clone(),
                BundleFile {
                    source: path,
                    relative,
                    mode: portable_mode(&metadata),
                    digest,
                },
            );
        }
    }
    if paths.is_empty() {
        return Err("Graph Turbo standalone bundle is empty".to_owned());
    }
    let mut root = blake3::Hasher::new();
    root.update(b"asp-graph-turbo-standalone-directory-v1\0");
    for file in paths.values() {
        root.update(file.relative.as_bytes());
        root.update(b"\0file\0");
        root.update(file.mode.to_string().as_bytes());
        root.update(b"\0");
        root.update(file.digest.as_bytes());
        root.update(b"\0");
    }
    Ok((
        format!("blake3-256:{}", root.finalize().to_hex()),
        paths.into_values().collect(),
    ))
}

async fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("failed to open artifact {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to read artifact {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

async fn publish_bundle_atomically(destination: &Path, files: &[BundleFile]) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "Graph Turbo artifact destination has no parent".to_owned())?;
    let stage = parent.join(format!(
        ".{GRAPH_TURBO_BUNDLE_NAME}.installing-{}",
        std::process::id()
    ));
    if tokio::fs::try_exists(&stage)
        .await
        .map_err(|error| format!("failed to inspect Graph Turbo staging path: {error}"))?
    {
        tokio::fs::remove_dir_all(&stage)
            .await
            .map_err(|error| format!("failed to remove stale Graph Turbo staging path: {error}"))?;
    }
    tokio::fs::create_dir(&stage)
        .await
        .map_err(|error| format!("failed to create Graph Turbo staging path: {error}"))?;
    for file in files {
        let target = stage.join(&file.relative);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!("failed to create Graph Turbo bundle directory: {error}")
            })?;
        }
        tokio::fs::copy(&file.source, &target)
            .await
            .map_err(|error| format!("failed to copy Graph Turbo bundle entry: {error}"))?;
        apply_mode(&target, file.mode).await?;
    }
    tokio::fs::rename(&stage, destination)
        .await
        .map_err(|error| format!("failed to atomically publish Graph Turbo bundle: {error}"))
}

async fn publish_config_atomically(
    protocol_home: &Path,
    execution_artifact_locator: &Path,
    execution_artifact_digest: &str,
    runtime_artifact_digest: &str,
) -> Result<(), String> {
    let server_root = protocol_home.join("runtime/server");
    tokio::fs::create_dir_all(&server_root)
        .await
        .map_err(|error| format!("failed to create Runtime Server state root: {error}"))?;
    let config_path = server_root.join(GRAPH_TURBO_CONFIG_NAME);
    let stage = server_root.join(format!(
        ".{GRAPH_TURBO_CONFIG_NAME}.installing-{}",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(&GraphTurboResidentConfig {
        schema_id: "agent.semantic-protocols.semantic-graph-turbo-resident-config",
        schema_version: "1",
        artifact_kind: "standalone-directory",
        execution_artifact_locator,
        execution_artifact_digest,
        runtime_artifact_digest,
    })
    .map_err(|error| format!("failed to encode Graph Turbo resident config: {error}"))?;
    let mut file = tokio::fs::File::create(&stage)
        .await
        .map_err(|error| format!("failed to create Graph Turbo resident config stage: {error}"))?;
    tokio::io::AsyncWriteExt::write_all(&mut file, &bytes)
        .await
        .map_err(|error| format!("failed to write Graph Turbo resident config: {error}"))?;
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|error| format!("failed to flush Graph Turbo resident config: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("failed to sync Graph Turbo resident config: {error}"))?;
    drop(file);
    tokio::fs::rename(&stage, &config_path)
        .await
        .map_err(|error| {
            format!("failed to atomically publish Graph Turbo resident config: {error}")
        })
}

#[cfg(unix)]
fn portable_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn portable_mode(_metadata: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
async fn apply_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(|error| format!("failed to preserve Graph Turbo artifact mode: {error}"))
}

#[cfg(not(unix))]
async fn apply_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact.rs"]
mod tests;
