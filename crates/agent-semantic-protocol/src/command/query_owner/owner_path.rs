use std::path::{Path, PathBuf};

pub(super) fn resolve_owner_path(
    project_root: &Path,
    locator_root: &Path,
    owner_path: &Path,
) -> Option<PathBuf> {
    let candidates = if owner_path.is_absolute() {
        vec![owner_path.to_path_buf()]
    } else {
        vec![locator_root.join(owner_path), project_root.join(owner_path)]
    };
    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(super) fn owner_path_is_file_like(path: &Path) -> bool {
    path.extension().is_some() || path.components().count() > 1
}
