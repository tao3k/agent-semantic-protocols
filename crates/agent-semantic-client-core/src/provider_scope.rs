// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Provider manifest path rules shared by client services.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::RuntimeProvider;

#[must_use]
/// Return whether a provider owns the source-file extension at `path`.
pub fn provider_supports_source_file(provider: &RuntimeProvider, path: &Path) -> bool {
    agent_semantic_config::source_extension::source_extensions_support_file(
        &provider.source_extensions,
        path,
    )
}

/// Return whether a provider excludes a path from its declared source scope.
#[must_use]
/// Resolve a project-scoped child path, including the project root itself.
pub fn project_child_path(project_root: &Path, path: &str) -> Option<PathBuf> {
    if path == "." || path.is_empty() {
        return Some(project_root.to_path_buf());
    }
    scoped_child_path(project_root, path)
}

/// Resolve a relative child without permitting absolute or parent traversal.
#[must_use]
pub fn scoped_child_path(root: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return None;
    }
    Some(root.join(path))
}

#[must_use]
/// Render `path` relative to the project root using canonical separators.
pub fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Normalize a project-relative path to slash-separated canonical text.
#[must_use]
pub fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}
