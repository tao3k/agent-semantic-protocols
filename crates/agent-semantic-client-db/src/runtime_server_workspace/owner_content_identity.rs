// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_owner_content_identity.rs"]
mod runtime_server_owner_content_identity_tests;

pub(super) struct RuntimeOwnerContentIdentity {
    pub digest: String,
}

pub(super) async fn read(
    project_root: &Path,
    owner_path: &str,
) -> Result<Option<RuntimeOwnerContentIdentity>, String> {
    let relative = normalized_owner_path(owner_path)?;
    let path = project_root.join(relative);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read runtime owner {}: {error}",
                path.display()
            ));
        }
    };
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(&bytes).value
    );
    Ok(Some(RuntimeOwnerContentIdentity { digest }))
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
