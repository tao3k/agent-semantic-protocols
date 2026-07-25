use std::path::{Path, PathBuf};

pub(super) fn for_workspace(root: &Path) -> PathBuf {
    crate::state_home_fixture::canonical_activation_path(
        root,
        &crate::state_home_fixture::default_state_home(root),
    )
}
