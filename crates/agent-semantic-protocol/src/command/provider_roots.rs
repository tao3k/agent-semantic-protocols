use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    if let Some(cache_home) =
        git_toplevel_cache_home(project_root).or_else(|| git_toplevel_cache_home(activation_root))
    {
        return Ok(cache_home);
    }
    env::var_os("PRJ_CACHE_HOME")
        .map(PathBuf::from)
        .map(canonical_or_existing)
        .ok_or_else(|| {
            format!(
                "no git toplevel was found for {}; set PRJ_CACHE_HOME only when running outside a git worktree",
                project_root.display()
            )
        })
}

pub(super) fn effective_project_root_and_args(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
    activation_root: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    if let Some((root, args)) =
        explicit_positional_project_root(language_id, args, invocation_root)?
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

pub(super) fn activation_storage_root(activation_path: &Path) -> PathBuf {
    activation_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn explicit_positional_project_root(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let mut selected = None;
    for (index, value) in args.iter().enumerate().rev() {
        if value.starts_with('-') || arg_is_option_value(args, index) {
            continue;
        }
        let path = PathBuf::from(value);
        let absolute = if path.is_absolute() {
            path
        } else {
            invocation_root.join(path)
        };
        let Some(selected_root) = positional_project_root(language_id, &absolute) else {
            continue;
        };
        if selected.is_some() {
            return Err("expected at most one PROJECT_ROOT argument".to_string());
        }
        selected = Some((index, selected_root));
    }
    let Some((index, selected_root)) = selected else {
        return Ok(None);
    };
    let mut normalized_args = args.to_vec();
    normalized_args.remove(index);
    Ok(Some((selected_root, normalized_args)))
}

fn positional_project_root(language_id: &str, path: &Path) -> Option<PathBuf> {
    if path
        .join(".cache/agent-semantic-protocol/hooks/activation.json")
        .is_file()
        || path
            .join(".cache/agent-semantic-protocol/client/cache-manifest.json")
            .is_file()
    {
        return Some(canonical_or_existing(path.to_path_buf()));
    }
    language_project_marker_root(language_id, path)
}

fn language_project_marker_root(language_id: &str, path: &Path) -> Option<PathBuf> {
    let marker_names = match language_id {
        "rust" => &["Cargo.toml"][..],
        "typescript" => &["tsconfig.json", "package.json"][..],
        "python" => &["pyproject.toml"][..],
        "julia" => &["Project.toml", "JuliaProject.toml"][..],
        _ => &[][..],
    };
    marker_names
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

fn invocation_root_is_provider_project(language_id: &str, invocation_root: &Path) -> bool {
    invocation_root
        .join(".cache/agent-semantic-protocol/hooks/activation.json")
        .is_file()
        || language_project_marker_root(language_id, invocation_root).is_some()
}

fn git_toplevel_cache_home(project_root: &Path) -> Option<PathBuf> {
    git_toplevel(project_root).map(|root| canonical_or_existing(root.join(".cache")))
}

fn git_toplevel(project_root: &Path) -> Option<PathBuf> {
    if let Some(root) = project_root
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
    {
        return Some(canonical_or_existing(root.to_path_buf()));
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(canonical_or_existing(PathBuf::from(path)))
    }
}

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}
