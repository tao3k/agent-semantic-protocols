//! Argument parsing for the public client CLI.

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct ParsedArgs {
    pub(crate) command: Option<String>,
    pub(crate) activation_root: PathBuf,
    pub(crate) project_root: PathBuf,
    pub(crate) forwarded_args: Vec<String>,
    pub(crate) receipt_json: bool,
    pub(crate) frontier_receipt_out: Option<PathBuf>,
}

pub(crate) fn parse_client_args(
    args: Vec<String>,
    cwd: PathBuf,
    language_id: Option<&str>,
) -> Result<ParsedArgs, String> {
    let mut command = None;
    let mut activation_root = cwd.clone();
    let mut project_root = cwd;
    let mut explicit_project_root = false;
    let mut forwarded_args = Vec::new();
    let mut receipt_json = false;
    let mut frontier_receipt_out = None;
    let mut iter = args.into_iter();
    if let Some(first) = iter.next() {
        command = Some(first);
    }
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--language" if language_id.is_none() => {
                return Err("--language has been removed; use asp <rust|typescript|python> <search|query|check> ...".to_string());
            }
            "--root" => {
                project_root = PathBuf::from(
                    iter.next()
                        .ok_or_else(|| "--root requires a value".to_string())?,
                );
                activation_root = project_root.clone();
                explicit_project_root = true;
            }
            "--receipt-json" => {
                receipt_json = true;
            }
            "--frontier-receipt-out" => {
                frontier_receipt_out =
                    Some(PathBuf::from(iter.next().ok_or_else(|| {
                        "--frontier-receipt-out requires a path".to_string()
                    })?));
            }
            _ => forwarded_args.push(arg),
        }
    }
    if !explicit_project_root
        && should_infer_positional_project_root(command.as_deref())
        && let Some(root) = positional_project_root(language_id, &forwarded_args, &project_root)
    {
        if forwarded_args.len() > 1
            && positional_project_root(
                language_id,
                &forwarded_args[..forwarded_args.len() - 1],
                &project_root,
            )
            .is_some()
        {
            return Err("expected at most one PROJECT_ROOT argument".to_string());
        }
        project_root = root;
        forwarded_args.pop();
    }
    Ok(ParsedArgs {
        command,
        activation_root,
        project_root,
        forwarded_args,
        receipt_json,
        frontier_receipt_out,
    })
}

fn should_infer_positional_project_root(command: Option<&str>) -> bool {
    matches!(command, Some("search" | "query" | "check"))
}

fn positional_project_root(
    language_id: Option<&str>,
    forwarded_args: &[String],
    cwd: &Path,
) -> Option<PathBuf> {
    let value = forwarded_args.last()?;
    if value.starts_with('-') {
        return None;
    }
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    if value == "." {
        return Some(canonical_or_existing(absolute));
    }
    if absolute
        .join(".cache/agent-semantic-protocol/hooks/activation.json")
        .is_file()
        || absolute
            .join(".cache/agent-semantic-protocol/client/cache-manifest.json")
            .is_file()
    {
        return Some(canonical_or_existing(absolute));
    }
    let language_id = language_id?;
    language_project_marker_root(language_id, &absolute)
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

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}
