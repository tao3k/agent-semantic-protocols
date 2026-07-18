use std::path::{Path, PathBuf};

use serde_json::Value;

pub(super) fn for_workspace(root: &Path) -> PathBuf {
    let state_home = root.join(".agent-semantic-protocols");
    let mut matches = Vec::new();
    collect_paths(&state_home, &mut matches);
    let canonical_root = canonical_path(root);
    matches.retain(|path| activation_matches_workspace(path, &canonical_root));
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "activation paths for {}: {matches:?}",
        canonical_root.display()
    );
    matches.remove(0)
}

fn activation_matches_workspace(path: &Path, canonical_root: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|activation| activation["projectRoot"].as_str().map(PathBuf::from))
        .is_some_and(|project_root| canonical_path(&project_root) == canonical_root)
}

fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn collect_paths(dir: &Path, matches: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_paths(&path, matches);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("activation.json")
            && path.parent().and_then(|parent| parent.file_name())
                == Some(std::ffi::OsStr::new("state"))
        {
            matches.push(path);
        }
    }
}
