use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeveloperArtifactCleanupReceipt {
    removed_entries: usize,
    artifact_tree_removed: bool,
}

impl DeveloperArtifactCleanupReceipt {
    #[must_use]
    pub fn removed_entries(&self) -> usize {
        self.removed_entries
    }

    #[must_use]
    pub fn artifact_tree_removed(&self) -> bool {
        self.artifact_tree_removed
    }
}

pub async fn remove_developer_artifact_lattice(
    runtime_bin_dir: &Path,
    artifact_root: &Path,
) -> Result<DeveloperArtifactCleanupReceipt, String> {
    let configured_artifact_root = artifact_root.to_path_buf();
    let artifact_root = match tokio::fs::canonicalize(artifact_root).await {
        Ok(path) => Some(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "canonicalize Developer artifact root {}: {error}",
                artifact_root.display()
            ));
        }
    };
    let mut removed_entries = 0;
    let mut legacy_prefixes = Vec::new();
    let mut possible_companions = Vec::new();
    let mut entries = tokio::fs::read_dir(runtime_bin_dir)
        .await
        .map_err(|error| {
            format!(
                "read Developer Runtime bin directory {}: {error}",
                runtime_bin_dir.display()
            )
        })?;
    while let Some(entry) = entries.next_entry().await.map_err(|error| {
        format!(
            "read Developer Runtime bin entry in {}: {error}",
            runtime_bin_dir.display()
        )
    })? {
        let metadata = entry.file_type().await.map_err(|error| {
            format!(
                "read Developer Runtime bin entry type {}: {error}",
                entry.path().display()
            )
        })?;
        if metadata.is_file() {
            possible_companions.push((entry.path(), false));
            continue;
        }
        if metadata.is_dir() {
            possible_companions.push((entry.path(), true));
            continue;
        }
        if !metadata.is_symlink() {
            continue;
        }
        let link_target = tokio::fs::read_link(entry.path()).await.map_err(|error| {
            format!(
                "read Developer Runtime stable link {}: {error}",
                entry.path().display()
            )
        })?;
        let resolved_target = absolute_link_target(entry.path(), link_target)?;
        let canonical_target = tokio::fs::canonicalize(&resolved_target).await.ok();
        let target_is_artifact = resolved_target.starts_with(&configured_artifact_root)
            || artifact_root
                .as_ref()
                .zip(canonical_target.as_ref())
                .is_some_and(|(artifact_root, target)| target.starts_with(artifact_root));
        if !target_is_artifact {
            continue;
        }
        if let Some(prefix) = entry.file_name().to_str() {
            legacy_prefixes.push(format!("{prefix}__"));
        }
        tokio::fs::remove_file(entry.path())
            .await
            .map_err(|error| {
                format!(
                    "remove legacy Developer artifact link {}: {error}",
                    entry.path().display()
                )
            })?;
        removed_entries += 1;
    }

    for (path, is_directory) in possible_companions {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !legacy_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        if is_directory {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_file(&path).await
        }
        .map_err(|error| {
            format!(
                "remove owned legacy Developer Runtime companion {}: {error}",
                path.display()
            )
        })?;
        removed_entries += 1;
    }

    let artifact_tree_removed = if let Some(artifact_root) = artifact_root {
        tokio::fs::remove_dir_all(&artifact_root)
            .await
            .map_err(|error| {
                format!(
                    "remove Developer artifact lattice {}: {error}",
                    artifact_root.display()
                )
            })?;
        true
    } else {
        false
    };
    Ok(DeveloperArtifactCleanupReceipt {
        removed_entries,
        artifact_tree_removed,
    })
}

fn absolute_link_target(link_path: PathBuf, target: PathBuf) -> Result<PathBuf, String> {
    if target.is_absolute() {
        return Ok(target);
    }
    let parent = link_path.parent().ok_or_else(|| {
        format!(
            "Developer Runtime stable link has no parent: {}",
            link_path.display()
        )
    })?;
    Ok(parent.join(target))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::remove_developer_artifact_lattice;

    #[tokio::test]
    async fn removes_only_links_into_developer_artifact_lattice() {
        let root = tempfile::tempdir().expect("tempdir");
        let runtime_bin = root.path().join("runtime/bin");
        let artifact_root = root.path().join("runtime/artifacts");
        let checkout_binary = root.path().join("checkout/target/debug/asp");
        let legacy_binary = artifact_root.join("blake3-256/old/asp-rust");
        tokio::fs::create_dir_all(&runtime_bin)
            .await
            .expect("runtime bin");
        tokio::fs::create_dir_all(legacy_binary.parent().expect("legacy parent"))
            .await
            .expect("legacy generation");
        tokio::fs::create_dir_all(checkout_binary.parent().expect("checkout parent"))
            .await
            .expect("checkout target");
        tokio::fs::write(&legacy_binary, b"legacy")
            .await
            .expect("legacy binary");
        tokio::fs::write(&checkout_binary, b"current")
            .await
            .expect("checkout binary");
        symlink(&legacy_binary, runtime_bin.join("asp-rust")).expect("legacy link");
        tokio::fs::write(
            runtime_bin.join("asp-rust__launcher.c"),
            b"legacy companion",
        )
        .await
        .expect("legacy companion");
        symlink(&checkout_binary, runtime_bin.join("asp")).expect("checkout link");

        let receipt = remove_developer_artifact_lattice(&runtime_bin, &artifact_root)
            .await
            .expect("clean Developer lattice");

        assert_eq!(receipt.removed_entries(), 2);
        assert!(receipt.artifact_tree_removed());
        assert!(!runtime_bin.join("asp-rust").exists());
        assert_eq!(
            tokio::fs::canonicalize(runtime_bin.join("asp"))
                .await
                .expect("checkout link remains"),
            tokio::fs::canonicalize(checkout_binary)
                .await
                .expect("checkout binary")
        );
        assert!(!artifact_root.exists());
    }

    #[tokio::test]
    async fn preserves_unowned_bin_files_when_artifact_lattice_is_already_absent() {
        let root = tempfile::tempdir().expect("tempdir");
        let runtime_bin = root.path().join("runtime/bin");
        let checkout_binary = root.path().join("checkout/target/debug/asp");
        tokio::fs::create_dir_all(&runtime_bin)
            .await
            .expect("runtime bin");
        tokio::fs::create_dir_all(checkout_binary.parent().expect("checkout parent"))
            .await
            .expect("checkout target");
        tokio::fs::write(&checkout_binary, b"current")
            .await
            .expect("checkout binary");
        tokio::fs::write(runtime_bin.join("legacy-generated.c"), b"legacy")
            .await
            .expect("legacy generated file");
        symlink(&checkout_binary, runtime_bin.join("asp")).expect("checkout link");

        let receipt =
            remove_developer_artifact_lattice(&runtime_bin, &root.path().join("runtime/artifacts"))
                .await
                .expect("clean Developer bin");

        assert_eq!(receipt.removed_entries(), 0);
        assert!(!receipt.artifact_tree_removed());
        assert!(runtime_bin.join("legacy-generated.c").exists());
        assert!(runtime_bin.join("asp").exists());
    }
}
