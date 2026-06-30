//! Provider manifest path rules shared by client services.

use std::path::{Component, Path, PathBuf};

use crate::ResolvedProvider;

#[must_use]
pub fn provider_supports_source_file(provider: &ResolvedProvider, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    provider.source_extensions.iter().any(|candidate| {
        candidate
            .trim_start_matches('.')
            .eq_ignore_ascii_case(extension)
    })
}

#[must_use]
pub fn provider_ignores_path(
    project_root: &Path,
    provider: &ResolvedProvider,
    path: &Path,
) -> bool {
    let relative = relative_project_path(project_root, path);
    provider.ignored_path_prefixes.iter().any(|prefix| {
        let prefix = normalize_project_path(prefix);
        relative == prefix || relative.starts_with(&format!("{prefix}/"))
    })
}

#[must_use]
pub fn project_child_path(project_root: &Path, path: &str) -> Option<PathBuf> {
    if path == "." || path.is_empty() {
        return Some(project_root.to_path_buf());
    }
    scoped_child_path(project_root, path)
}

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
pub fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

#[must_use]
pub fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}
