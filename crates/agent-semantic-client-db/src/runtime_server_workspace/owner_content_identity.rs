// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::{Path, PathBuf};

pub(super) struct RuntimeOwnerContentIdentity {
    pub digest: String,
    pub bytes: Vec<u8>,
}

pub(super) async fn read(
    project_root: &Path,
    owner_path: &str,
) -> Result<Option<RuntimeOwnerContentIdentity>, String> {
    let relative = normalized_owner_path(owner_path)?;
    let path = project_root.join(relative);
    let before = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read runtime owner {}: {error}",
                path.display()
            ));
        }
    };
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("failed to read runtime owner {}: {error}", path.display()))?;
    let after = tokio::fs::metadata(&path).await.map_err(|error| {
        format!(
            "failed to restat runtime owner {} after read: {error}",
            path.display()
        )
    })?;
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || after.len() != bytes.len() as u64
    {
        return Err(format!(
            "runtime owner changed during content acquisition: {}",
            path.display()
        ));
    }
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(&bytes).value
    );
    Ok(Some(RuntimeOwnerContentIdentity { digest, bytes }))
}

pub(super) fn normalized_owner_path(owner_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(owner_path);
    if relative.is_absolute()
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("runtime owner freshness requires a normalized relative owner path".to_owned());
    }
    Ok(relative.to_path_buf())
}
#[cfg(test)]
#[path = "../../tests/unit/runtime_server_owner_content_identity.rs"]
mod tests;
