use std::path::{Path, PathBuf};

pub(super) fn collect_non_symlink_scope_files(
    scope_root: &Path,
    limit: usize,
    mut ignores_path: impl FnMut(&Path) -> bool,
    mut matches_file: impl FnMut(&Path) -> bool,
) -> Result<Vec<PathBuf>, String> {
    if limit == 0 || !scope_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut pending_directories = vec![scope_root.to_path_buf()];
    while let Some(directory) = pending_directories.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|err| {
            format!(
                "failed to read provider fallback scope {}: {err}",
                directory.display()
            )
        })?;
        let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|err| {
            format!(
                "failed to read provider fallback entry under {}: {err}",
                directory.display()
            )
        })?;
        entries.sort_by_key(std::fs::DirEntry::file_name);

        let mut child_directories = Vec::new();
        for entry in entries {
            let path = entry.path();
            if ignores_path(&path) {
                continue;
            }
            let file_type = entry.file_type().map_err(|err| {
                format!(
                    "failed to classify provider fallback entry {}: {err}",
                    path.display()
                )
            })?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                child_directories.push(path);
            } else if file_type.is_file() && matches_file(&path) {
                files.push(path);
            }
            if files.len() >= limit {
                return Ok(files);
            }
        }
        pending_directories.extend(child_directories.into_iter().rev());
    }
    Ok(files)
}

#[cfg(all(test, unix))]
#[path = "tests/scope_walk.rs"]
mod tests;
