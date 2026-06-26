use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_runtime::{
    project_cache_home_for_roots, project_local_activation_path,
    project_local_client_cache_manifest_path, project_root_for_activation_path,
};

use super::search_language_files::language_file_spec;

pub(super) fn activation_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let configured = PathBuf::from(project_root);
    let root = if configured.is_absolute() {
        configured
    } else {
        activation_storage_root(activation_path).join(configured)
    };
    fs::canonicalize(&root).unwrap_or(root)
}

pub(super) fn client_backend_cache_home(
    activation_root: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    project_cache_home_for_roots(activation_root, project_root)
}

pub(super) fn effective_project_root_and_args(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
    activation_root: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    validate_code_flag_boundary(args)?;
    if let Some((workspace_root, normalized_args)) =
        explicit_workspace_project_root(language_id, args, invocation_root)?
    {
        return Ok((workspace_root, normalized_args));
    }
    if let Some((root, args)) =
        explicit_positional_project_root(language_id, args, invocation_root, activation_root)?
    {
        return Ok((root, args));
    }

    if invocation_root != activation_root
        && invocation_root.starts_with(activation_root)
        && invocation_root_is_provider_project(language_id, invocation_root)
    {
        return Ok((invocation_root.to_path_buf(), args.to_vec()));
    }

    if args.last().is_some_and(|arg| arg == ".")
        && invocation_root_is_provider_project(language_id, invocation_root)
    {
        Ok((invocation_root.to_path_buf(), args.to_vec()))
    } else {
        Ok((activation_root.to_path_buf(), args.to_vec()))
    }
}

pub(super) fn validate_explicit_workspace_project_root(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
) -> Result<(), String> {
    explicit_workspace_project_root(language_id, args, invocation_root).map(|_| ())
}

fn validate_code_flag_boundary(args: &[String]) -> Result<(), String> {
    if !matches!(args.first().map(String::as_str), Some("query" | "search")) {
        return Ok(());
    }
    for window in args.windows(2) {
        if window[0] == "--code" && !window[1].starts_with('-') {
            return Err(
                "query/search --code does not accept a trailing PROJECT_ROOT; use --workspace PROJECT_ROOT"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn explicit_workspace_project_root(
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
            return Err("--workspace requires a project root".to_string());
        };
        if value.starts_with('-') {
            return Err("--workspace requires a project root".to_string());
        }
        if selected.is_some() {
            return Err("expected at most one --workspace PROJECT_ROOT argument".to_string());
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
        return Ok(());
    }
    if metadata.is_file() {
        return Err(format!(
            "--workspace requires a directory project root, got file `{}`. Keep the file path as the owner/selector and use a directory workspace, for example `asp {language_id} search owner <file> items --query '<terms>' --workspace . --view seeds`.",
            root.display()
        ));
    }
    Err(format!(
        "--workspace requires a directory project root, got non-directory `{}`",
        root.display()
    ))
}

fn workspace_bounded_root(root: PathBuf, activation_root: &Path) -> Result<PathBuf, String> {
    let workspace_root = canonical_or_existing(activation_root.to_path_buf());
    if root.starts_with(&workspace_root) {
        return Ok(root);
    }
    Err(format!(
        "project root `{}` is outside workspace `{}`",
        root.display(),
        workspace_root.display()
    ))
}

pub(super) fn activation_storage_root(activation_path: &Path) -> PathBuf {
    project_root_for_activation_path(activation_path).unwrap_or_else(|| {
        activation_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    })
}

fn explicit_positional_project_root(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
    activation_root: &Path,
) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let mut selected = None;
    let check_command = args.first().is_some_and(|command| command == "check");
    let query_owner_arg_index = first_query_owner_arg_index(args);
    for (index, value) in args.iter().enumerate().rev() {
        if Some(index) == query_owner_arg_index {
            continue;
        }
        if value.starts_with('-') || arg_is_option_value(args, index) {
            continue;
        }
        let path = PathBuf::from(value);
        let absolute = if path.is_absolute() {
            path
        } else {
            invocation_root.join(path)
        };
        let Some(selected_root) = positional_project_root(language_id, &absolute, check_command)
        else {
            continue;
        };
        if selected.is_some() {
            return Err("expected at most one PROJECT_ROOT argument".to_string());
        }
        selected = Some((
            index,
            workspace_bounded_root(selected_root, activation_root)?,
        ));
    }
    let Some((index, selected_root)) = selected else {
        return Ok(None);
    };
    let mut normalized_args = args.to_vec();
    normalized_args.remove(index);
    Ok(Some((selected_root, normalized_args)))
}

fn first_query_owner_arg_index(args: &[String]) -> Option<usize> {
    if args.first().map(String::as_str) != Some("query") {
        return None;
    }
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg.starts_with("--") {
            index += if arg_contains_value(arg) || !option_takes_value(arg) {
                1
            } else {
                2
            };
            continue;
        }
        if arg.starts_with('-') || arg == "." {
            index += 1;
            continue;
        }
        return Some(index);
    }
    None
}

fn positional_project_root(language_id: &str, path: &Path, check_command: bool) -> Option<PathBuf> {
    let activation_path = project_local_activation_path(path);
    if activation_path.is_file() || project_local_client_cache_manifest_path(path).is_file() {
        return Some(canonical_or_existing(path.to_path_buf()));
    }
    if check_command && path.is_dir() {
        return Some(canonical_or_existing(path.to_path_buf()));
    }
    language_project_marker_root(language_id, path)
}

fn language_project_marker_root(language_id: &str, path: &Path) -> Option<PathBuf> {
    let file_spec = language_file_spec(language_id);
    file_spec
        .project_markers()
        .iter()
        .find_map(|marker| marker_root_for(path, marker))
}

fn marker_root_for(path: &Path, marker_name: &str) -> Option<PathBuf> {
    if path.file_name().and_then(|name| name.to_str()) == Some(marker_name) && path.is_file() {
        return path
            .parent()
            .map(Path::to_path_buf)
            .map(canonical_or_existing);
    }
    path.join(marker_name)
        .is_file()
        .then(|| canonical_or_existing(path.to_path_buf()))
}

fn arg_is_option_value(args: &[String], index: usize) -> bool {
    let Some(previous) = index.checked_sub(1).and_then(|previous| args.get(previous)) else {
        return false;
    };
    if !previous.starts_with("--") || previous.contains('=') {
        return false;
    }
    !matches!(
        previous.as_str(),
        "--changed" | "--code" | "--full" | "--json" | "--names-only" | "--receipt-json"
    )
}

fn arg_contains_value(arg: &str) -> bool {
    arg.contains('=')
}

fn option_takes_value(arg: &str) -> bool {
    !matches!(
        arg,
        "--changed" | "--code" | "--full" | "--json" | "--names-only" | "--receipt-json"
    )
}

fn invocation_root_is_provider_project(language_id: &str, invocation_root: &Path) -> bool {
    project_local_activation_path(invocation_root).is_file()
        || language_project_marker_root(language_id, invocation_root).is_some()
}

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}
