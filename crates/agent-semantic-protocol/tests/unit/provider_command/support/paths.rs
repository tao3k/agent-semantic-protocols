use std::path::Path;

pub(in crate::provider_command) fn org_artifact_target(root: &Path, relative: &str) -> String {
    super::artifacts_root(root)
        .join("org")
        .join(relative)
        .display()
        .to_string()
}
