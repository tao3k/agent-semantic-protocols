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
    let invocation_root = cwd;
    let activation_root = invocation_root.clone();
    let mut project_root = invocation_root.clone();
    let mut explicit_workspace = false;
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
            "--workspace" if accepts_workspace_flag(command.as_deref()) => {
                if explicit_workspace {
                    return Err("expected at most one --workspace argument".to_string());
                }
                let value = iter
                    .next()
                    .ok_or_else(|| "--workspace requires a project root".to_string())?;
                if value.starts_with('-') {
                    return Err("--workspace requires a project root".to_string());
                }
                project_root = resolve_project_root(&value, &invocation_root);
                explicit_workspace = true;
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
            _ if accepts_workspace_flag(command.as_deref()) && arg.starts_with("--workspace=") => {
                if explicit_workspace {
                    return Err("expected at most one --workspace argument".to_string());
                }
                let value = arg.strip_prefix("--workspace=").expect("workspace prefix");
                if value.is_empty() || value.starts_with('-') {
                    return Err("--workspace requires a project root".to_string());
                }
                project_root = resolve_project_root(value, &invocation_root);
                explicit_workspace = true;
            }
            _ => forwarded_args.push(arg),
        }
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

fn accepts_workspace_flag(command: Option<&str>) -> bool {
    matches!(command, Some("search" | "query" | "check"))
}

fn resolve_project_root(value: &str, invocation_root: &Path) -> PathBuf {
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        invocation_root.join(path)
    };
    canonical_or_existing(absolute)
}

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}
