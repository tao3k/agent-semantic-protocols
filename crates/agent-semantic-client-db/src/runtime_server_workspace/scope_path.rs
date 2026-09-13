// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical filesystem identity for one resident workspace source scope.

use std::path::{Path, PathBuf};

const ACTIVE_GENERATION_POINTER: &str = "active-generation.pointer";

pub fn workspace_generation_directory(
    workspace_store_root: &Path,
    workspace_identity: &str,
    canonical_project_root: &Path,
) -> Result<PathBuf, String> {
    if workspace_identity.trim().is_empty() {
        return Err("runtime workspace identity must be non-empty".to_owned());
    }
    if !canonical_project_root.is_absolute() {
        return Err(format!(
            "runtime workspace project root must be canonical and absolute: {}",
            canonical_project_root.display()
        ));
    }
    let project_root_key = canonical_project_root.to_string_lossy();
    let scope_id = format!(
        "scope-{}",
        &blake3::hash(project_root_key.as_bytes()).to_hex()[..16]
    );
    Ok(workspace_store_root
        .join(workspace_identity)
        .join("scopes")
        .join(scope_id)
        .join("generations"))
}

pub fn workspace_generation_pointer_path(
    workspace_store_root: &Path,
    workspace_identity: &str,
    canonical_project_root: &Path,
) -> Result<PathBuf, String> {
    Ok(workspace_generation_directory(
        workspace_store_root,
        workspace_identity,
        canonical_project_root,
    )?
    .join(ACTIVE_GENERATION_POINTER))
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_scope_path.rs"]
mod tests;
