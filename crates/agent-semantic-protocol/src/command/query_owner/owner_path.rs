use std::path::{Path, PathBuf};

pub(super) fn resolve_owner_path(project_root: &Path, owner_path: &Path) -> Option<PathBuf> {
    let canonical_project_root = project_root.canonicalize().ok()?;
    let candidate = if owner_path.is_absolute() {
        owner_path.to_path_buf()
    } else {
        canonical_project_root.join(owner_path)
    };
    let canonical_candidate = candidate.canonicalize().ok()?;
    (canonical_candidate.starts_with(&canonical_project_root) && canonical_candidate.is_file())
        .then_some(canonical_candidate)
}

pub(super) fn owner_path_is_file_like(path: &Path) -> bool {
    path.extension().is_some() || path.components().count() > 1
}
