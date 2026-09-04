//! Workspace selector validation for server-routed provider requests.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(super) fn explicit_workspace_project_root(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let mut selected = None::<PathBuf>;
    let mut normalized_args = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] != "--workspace" {
            normalized_args.push(args[index].clone());
            index += 1;
            continue;
        }
        let Some(value) = args.get(index + 1) else {
            return Err("--workspace requires a project root".to_owned());
        };
        if value.starts_with('-') {
            return Err("--workspace requires a project root".to_owned());
        }
        if selected.is_some() {
            return Err("expected at most one --workspace argument".to_owned());
        }
        let path = PathBuf::from(value);
        let absolute = if path.is_absolute() {
            path
        } else {
            invocation_root.join(path)
        };
        let root = canonical_or_existing(absolute);
        validate_workspace_root(language_id, &root)?;
        selected = Some(root);
        index += 2;
    }
    Ok(selected.map(|root| (root, normalized_args)))
}

fn validate_workspace_root(language_id: &str, root: &Path) -> Result<(), String> {
    let metadata = fs::metadata(root).map_err(|error| {
        format!(
            "--workspace project root does not exist or cannot be read: `{}`: {error}",
            root.display()
        )
    })?;
    if metadata.is_dir() {
        reject_asp_state_workspace_root(root)?;
        return Ok(());
    }
    if metadata.is_file() {
        return Err(format!(
            "--workspace requires a directory project root, got file `{}`. Keep the file path as the search scope and use a directory workspace, for example `asp {language_id} search '<terms>' --scope owner:<file> --workspace .`.",
            root.display()
        ));
    }
    Err(format!(
        "--workspace requires a directory project root, got non-directory `{}`",
        root.display()
    ))
}

fn reject_asp_state_workspace_root(root: &Path) -> Result<(), String> {
    let Some(project_state_root) = asp_project_state_root_ancestor(root) else {
        return Ok(());
    };
    if !project_state_root.join("project.json").is_file() {
        return Err(format!(
            "--workspace points inside malformed ASP project state root `{}`; expected `project.json` at `{}`. Use the real checkout workspace instead of ASP project state.",
            project_state_root.display(),
            project_state_root.join("project.json").display()
        ));
    }
    Err(format!(
        "--workspace points inside ASP project state root `{}`. ASP state roots are not provider workspaces; use the real checkout workspace.",
        project_state_root.display()
    ))
}

fn asp_project_state_root_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors().find_map(|ancestor| {
        let repo_dir = ancestor.file_name()?.to_str()?;
        if !repo_dir.starts_with("repo-") {
            return None;
        }
        let by_id_dir = ancestor.parent()?.file_name()?.to_str()?;
        if by_id_dir != "by-id" {
            return None;
        }
        let projects_dir = ancestor.parent()?.parent()?.file_name()?.to_str()?;
        (projects_dir == "projects").then(|| ancestor.to_path_buf())
    })
}

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}
