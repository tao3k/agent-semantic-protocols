use std::path::{Component, Path};

pub(crate) fn normalized_relative_directory(
    workspace_root: &Path,
    manifest_dir: &Path,
) -> Result<String, String> {
    let relative = manifest_dir.strip_prefix(workspace_root).map_err(|_| {
        format!(
            "CARGO_MANIFEST_DIR `{}` is outside workspace root `{}`",
            manifest_dir.display(),
            workspace_root.display()
        )
    })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "invalid member package directory `{}`",
                    relative.display()
                ));
            }
        }
    }
    if segments.is_empty() {
        return Ok(".".to_string());
    }
    Ok(segments.join("/"))
}
