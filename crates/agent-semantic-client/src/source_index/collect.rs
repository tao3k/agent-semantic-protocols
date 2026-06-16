//! Project-scope discovery for Rust SQL source-index refresh.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::cargo_workspace::source_index_cargo_workspace;
use super::config::{
    SOURCE_INDEX_CONFIG_FILENAMES, SOURCE_INDEX_EXTENSIONS, SOURCE_INDEX_FILE_LIMIT,
    SOURCE_INDEX_PROJECT_ANCHOR_FILENAMES, SOURCE_INDEX_SKIP_DIRS,
};
use super::model::{SourceIndexProjectAnchor, SourceIndexProjectKind};

pub(super) fn collect_source_index_files(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    let excluded_roots = source_index_excluded_project_roots(project_root);
    let anchors = discover_source_index_project_anchors(project_root, &excluded_roots)?;
    let mut files = BTreeSet::new();
    for anchor in anchors {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        collect_source_index_project_files(project_root, &anchor, &excluded_roots, &mut files)?;
    }
    let mut files: Vec<_> = files.into_iter().collect();
    files.truncate(SOURCE_INDEX_FILE_LIMIT);
    Ok(files)
}

fn discover_source_index_project_anchors(
    project_root: &Path,
    excluded_roots: &BTreeSet<PathBuf>,
) -> Result<Vec<SourceIndexProjectAnchor>, String> {
    let mut anchors = Vec::new();
    collect_source_index_project_anchor_at_root(project_root, project_root, &mut anchors);
    if let Some(member_roots) =
        source_index_cargo_workspace_member_roots(project_root, excluded_roots)
    {
        for member_root in member_roots {
            collect_source_index_project_anchor_at_root(project_root, &member_root, &mut anchors);
        }
    } else {
        collect_source_index_project_anchors_in(
            project_root,
            project_root,
            excluded_roots,
            &mut anchors,
        )?;
    }
    anchors.sort_by(|left, right| {
        left.root
            .cmp(&right.root)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });
    anchors.dedup_by(|left, right| {
        left.root == right.root
            && left.kind == right.kind
            && left.manifest_path == right.manifest_path
    });
    Ok(anchors)
}

fn collect_source_index_project_anchor_at_root(
    project_root: &Path,
    root: &Path,
    anchors: &mut Vec<SourceIndexProjectAnchor>,
) {
    for filename in SOURCE_INDEX_PROJECT_ANCHOR_FILENAMES {
        let path = root.join(filename);
        if path.is_file()
            && let Some(kind) = source_index_project_kind(&path)
        {
            anchors.push(SourceIndexProjectAnchor {
                root: path.parent().unwrap_or(project_root).to_path_buf(),
                manifest_path: path,
                kind,
            });
        }
    }
}

fn collect_source_index_project_anchors_in(
    project_root: &Path,
    dir: &Path,
    excluded_roots: &BTreeSet<PathBuf>,
    anchors: &mut Vec<SourceIndexProjectAnchor>,
) -> Result<(), String> {
    if should_skip_source_index_dir(project_root, dir, excluded_roots) {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "failed to read source index anchor dir {}: {error}",
                dir.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read source index anchor entry under {}: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect source index anchor path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_source_index_project_anchors_in(project_root, &path, excluded_roots, anchors)?;
        } else if file_type.is_file()
            && let Some(kind) = source_index_project_kind(&path)
        {
            anchors.push(SourceIndexProjectAnchor {
                root: path.parent().unwrap_or(project_root).to_path_buf(),
                manifest_path: path,
                kind,
            });
        }
    }
    Ok(())
}

fn source_index_project_kind(path: &Path) -> Option<SourceIndexProjectKind> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => Some(SourceIndexProjectKind::Rust),
        Some("pyproject.toml") => Some(SourceIndexProjectKind::Python),
        Some("package.json") => Some(SourceIndexProjectKind::TypeScript),
        Some("Project.toml") => Some(SourceIndexProjectKind::Julia),
        Some("gerbil.pkg") => Some(SourceIndexProjectKind::Gerbil),
        _ => None,
    }
}

fn collect_source_index_project_files(
    project_root: &Path,
    anchor: &SourceIndexProjectAnchor,
    excluded_roots: &BTreeSet<PathBuf>,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    collect_source_index_package_root_files(&anchor.root, files)?;
    if anchor.kind == SourceIndexProjectKind::Gerbil {
        collect_source_index_files_in(project_root, &anchor.root, excluded_roots, files)?;
        return Ok(());
    }
    for source_dir in source_index_project_source_dirs(anchor.kind) {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = anchor.root.join(source_dir);
        if path.is_dir() {
            collect_source_index_files_in(project_root, &path, excluded_roots, files)?;
        }
    }
    Ok(())
}

fn collect_source_index_package_root_files(
    package_root: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if files.len() >= SOURCE_INDEX_FILE_LIMIT {
        return Ok(());
    }
    let mut entries = fs::read_dir(package_root)
        .map_err(|error| {
            format!(
                "failed to read source index package root {}: {error}",
                package_root.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read source index package entry under {}: {error}",
                package_root.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect source index package path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_file() && supported_source_index_file(&path) {
            files.insert(path);
        }
    }
    Ok(())
}

fn source_index_project_source_dirs(kind: SourceIndexProjectKind) -> &'static [&'static str] {
    match kind {
        SourceIndexProjectKind::Rust => &["src", "tests", "benches", "examples"],
        SourceIndexProjectKind::Python => &["src", "tests", "test", "scripts"],
        SourceIndexProjectKind::TypeScript => &["src", "tests", "test", "scripts"],
        SourceIndexProjectKind::Julia => &["src", "test", "docs", "examples"],
        SourceIndexProjectKind::Gerbil => &[],
    }
}

fn collect_source_index_files_in(
    project_root: &Path,
    dir: &Path,
    excluded_roots: &BTreeSet<PathBuf>,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if files.len() >= SOURCE_INDEX_FILE_LIMIT
        || should_skip_source_index_dir(project_root, dir, excluded_roots)
    {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read source index dir {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read source index entry under {}: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect source index path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_source_index_files_in(project_root, &path, excluded_roots, files)?;
        } else if file_type.is_file() && supported_source_index_file(&path) {
            files.insert(path);
        }
    }
    Ok(())
}

fn should_skip_source_index_dir(
    project_root: &Path,
    dir: &Path,
    excluded_roots: &BTreeSet<PathBuf>,
) -> bool {
    let project_key = source_index_compare_path(project_root);
    let dir_key = source_index_compare_path(dir);
    if dir_key == project_key {
        return false;
    }
    if excluded_roots.contains(&dir_key) || is_nested_vcs_root(&project_key, &dir_key, dir) {
        return true;
    }
    dir.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SOURCE_INDEX_SKIP_DIRS.contains(&name))
}

fn source_index_excluded_project_roots(project_root: &Path) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::new();
    if let Some(workspace) = source_index_cargo_workspace(project_root) {
        for exclude in workspace.excludes {
            roots.insert(source_index_compare_path(&project_root.join(exclude)));
        }
    }
    roots
}

fn source_index_cargo_workspace_member_roots(
    project_root: &Path,
    excluded_roots: &BTreeSet<PathBuf>,
) -> Option<BTreeSet<PathBuf>> {
    let workspace = source_index_cargo_workspace(project_root)?;
    let members = workspace.members;
    let excluded_members = workspace.excludes;
    let mut roots = BTreeSet::new();
    for member in members {
        if excluded_members.contains(&member) {
            continue;
        }
        for root in expand_source_index_cargo_member(project_root, &member) {
            let key = source_index_compare_path(&root);
            if !excluded_roots.contains(&key) {
                roots.insert(root);
            }
        }
    }
    Some(roots)
}

fn expand_source_index_cargo_member(project_root: &Path, member: &str) -> Vec<PathBuf> {
    if let Some(prefix) = member.strip_suffix("/*") {
        let base = project_root.join(prefix);
        return fs::read_dir(base)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
    }
    vec![project_root.join(member)]
}

fn source_index_compare_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_nested_vcs_root(project_key: &Path, dir_key: &Path, dir: &Path) -> bool {
    dir_key != project_key && dir.join(".git").exists()
}

fn supported_source_index_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SOURCE_INDEX_EXTENSIONS.contains(&extension))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                SOURCE_INDEX_PROJECT_ANCHOR_FILENAMES.contains(&name)
                    || SOURCE_INDEX_CONFIG_FILENAMES.contains(&name)
            })
}
