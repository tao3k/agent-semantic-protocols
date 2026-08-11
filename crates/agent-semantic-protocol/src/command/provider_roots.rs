use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE,
};
use agent_semantic_runtime::{
    project_activation_path, project_root_for_activation_path, project_state_paths,
};

use super::provider_fast_path::search_owner_items_owner_path;
use agent_semantic_search::language_file_spec;

pub(super) fn activation_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let configured = PathBuf::from(project_root);
    let root = if configured.is_absolute() {
        configured
    } else {
        activation_storage_root(activation_path).join(configured)
    };
    fs::canonicalize(&root).unwrap_or(root)
}


pub(super) fn effective_project_root_and_args(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
    activation_root: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    validate_facade_view_args(args)?;
    let args = args.to_vec();
    if let Some((workspace_root, normalized_args)) =
        explicit_workspace_project_root(language_id, &args, invocation_root)?
    {
        let (workspace_root, normalized_args) =
            rebase_structural_selector_to_member_root(language_id, workspace_root, normalized_args);
        return Ok(rebase_search_owner_to_member_root(
            language_id,
            workspace_root,
            normalized_args,
        ));
    }
    if let Some((root, args)) =
        explicit_positional_project_root(language_id, &args, invocation_root, activation_root)?
    {
        return Ok((root, args));
    }

    if invocation_root != activation_root
        && invocation_root.starts_with(activation_root)
        && invocation_root_is_provider_project(language_id, invocation_root)
    {
        return Ok((invocation_root.to_path_buf(), args));
    }

    if args.last().is_some_and(|arg| arg == ".") && trailing_dot_is_context_root_only(&args) {
        let mut normalized_args = args;
        normalized_args.pop();
        let project_root = if invocation_root_is_provider_project(language_id, invocation_root) {
            invocation_root
        } else {
            activation_root
        };
        Ok((project_root.to_path_buf(), normalized_args))
    } else if args.last().is_some_and(|arg| arg == ".")
        && invocation_root_is_provider_project(language_id, invocation_root)
    {
        Ok((invocation_root.to_path_buf(), args))
    } else {
        Ok((activation_root.to_path_buf(), args))
    }
}

fn rebase_search_owner_to_member_root(
    language_id: &str,
    workspace_root: PathBuf,
    mut args: Vec<String>,
) -> (PathBuf, Vec<String>) {
    let Some(owner) = search_owner_items_owner_path(&args).map(str::to_owned) else {
        return (workspace_root, args);
    };
    let owner_path = PathBuf::from(&owner);
    let Some(member_root) =
        provider_member_root_for_owner(language_id, &workspace_root, &owner_path)
            .filter(|root| root != &workspace_root)
    else {
        return (workspace_root, args);
    };
    let absolute_owner = workspace_root.join(&owner_path);
    let Ok(member_owner) = absolute_owner.strip_prefix(&member_root) else {
        return (workspace_root, args);
    };
    let member_owner = member_owner.to_string_lossy().replace('\\', "/");
    if let Some(owner_argument) = args.iter_mut().find(|argument| argument.as_str() == owner) {
        *owner_argument = member_owner;
    }
    (member_root, args)
}

pub(super) fn provider_member_root_for_owner(
    language_id: &str,
    workspace_root: &Path,
    owner: &Path,
) -> Option<PathBuf> {
    if owner.is_absolute()
        || owner.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let absolute_owner = workspace_root.join(owner);
    let marker_start = if absolute_owner.is_dir() {
        absolute_owner.as_path()
    } else {
        absolute_owner.parent().unwrap_or(workspace_root)
    };
    marker_start
        .ancestors()
        .take_while(|candidate| candidate.starts_with(workspace_root))
        .find_map(|candidate| language_project_marker_root(language_id, candidate))
}

fn rebase_structural_selector_to_member_root(
    language_id: &str,
    workspace_root: PathBuf,
    mut args: Vec<String>,
) -> (PathBuf, Vec<String>) {
    let selector_prefix = format!("{language_id}://");
    let selector = args.iter().enumerate().find_map(|(index, arg)| {
        if arg == "--selector" {
            return args
                .get(index + 1)
                .map(|value| (index + 1, value.as_str(), false));
        }
        arg.strip_prefix("--selector=")
            .map(|value| (index, value, true))
    });
    let Some((selector_index, selector, inline)) = selector else {
        return (workspace_root, args);
    };
    let Some(selector_body) = selector.strip_prefix(&selector_prefix) else {
        return (workspace_root, args);
    };
    let Some((owner, fragment)) = selector_body.split_once('#') else {
        return (workspace_root, args);
    };
    let owner = PathBuf::from(owner);
    let absolute_owner = workspace_root.join(&owner);
    let member_root = provider_member_root_for_owner(language_id, &workspace_root, &owner);
    let Some(member_root) = member_root.filter(|root| root != &workspace_root) else {
        return (workspace_root, args);
    };
    let Ok(member_owner) = absolute_owner.strip_prefix(&member_root) else {
        return (workspace_root, args);
    };
    let member_owner = member_owner.to_string_lossy().replace('\\', "/");
    let rebased = format!("{selector_prefix}{member_owner}#{fragment}");
    args[selector_index] = if inline {
        format!("--selector={rebased}")
    } else {
        rebased
    };
    (member_root, args)
}

fn trailing_dot_is_context_root_only(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("ast-patch" | "guide")
    )
}

fn validate_facade_view_args(args: &[String]) -> Result<(), String> {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--view" {
            let Some(value) = args.get(index + 1) else {
                return Err(if args.iter().any(|arg| arg == "lexical") {
                    "search lexical --view requires seeds"
                } else {
                    "roots --view requires a value"
                }
                .to_string());
            };
            if value.starts_with('-') {
                return Err(if args.iter().any(|arg| arg == "lexical") {
                    "search lexical --view requires seeds"
                } else {
                    "roots --view requires a value"
                }
                .to_string());
            }
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--view=") {
            if value.is_empty() {
                return Err(if args.iter().any(|arg| arg == "lexical") {
                    "search lexical --view requires seeds"
                } else {
                    "roots --view requires a value"
                }
                .to_string());
            }
            index += 1;
            continue;
        }
        index += 1;
    }
    Ok(())
}


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
            return Err("--workspace requires a project root".to_string());
        };
        if value.starts_with('-') {
            return Err("--workspace requires a project root".to_string());
        }
        if selected.is_some() {
            return Err("expected at most one --workspace argument".to_string());
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
            "--workspace requires a directory project root, got file `{}`. Keep the file path as the owner/selector and use a directory workspace, for example `asp {language_id} search owner <file> items --query '<terms>' --workspace . --view seeds`.",
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
            "--workspace points inside malformed ASP project state root `{}`; expected `project.json` at `{}`. Use the real checkout workspace instead of `.agent-semantic-protocols/projects/by-id/...`.",
            project_state_root.display(),
            project_state_root.join("project.json").display()
        ));
    }
    Err(format!(
        "--workspace points inside ASP project state root `{}`. ASP state roots are not provider workspaces; use the real checkout workspace and let rollout/session registry map commands to repo/workspace artifacts.",
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
        if projects_dir != "projects" {
            return None;
        }
        Some(ancestor.to_path_buf())
    })
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
    if args.first().is_some_and(|command| command == "search") {
        validate_search_positional_project_roots(language_id, args, invocation_root)?;
        return Ok(None);
    }
    let mut selected = None;
    let check_command = args.first().is_some_and(|command| command == "check");
    let context_root_only_command = args
        .first()
        .is_some_and(|command| matches!(command.as_str(), "ast-patch" | "guide"));
    let query_owner_arg_index = first_query_owner_arg_index(args);
    for (index, value) in args.iter().enumerate().rev() {
        if index == 0 {
            continue;
        }
        if Some(index) == query_owner_arg_index {
            continue;
        }
        if is_search_view_arg(args, index) {
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
        if context_root_only_command {
            break;
        }
    }
    let Some((index, selected_root)) = selected else {
        return Ok(None);
    };
    let mut normalized_args = args.to_vec();
    normalized_args.remove(index);
    Ok(Some((selected_root, normalized_args)))
}

fn validate_search_positional_project_roots(
    language_id: &str,
    args: &[String],
    invocation_root: &Path,
) -> Result<(), String> {
    let mut roots = 0usize;
    for (index, value) in args.iter().enumerate().rev() {
        if index == 0 || value.starts_with('-') || arg_is_option_value(args, index) {
            break;
        }
        let path = PathBuf::from(value);
        let absolute = if path.is_absolute() {
            path
        } else {
            invocation_root.join(path)
        };
        if search_scope_project_root(language_id, value, &absolute).is_some() {
            roots += 1;
            if roots > 1 {
                return Err("expected at most one PROJECT_ROOT argument".to_string());
            }
        }
    }
    Ok(())
}

fn search_scope_project_root(language_id: &str, raw: &str, path: &Path) -> Option<PathBuf> {
    if raw == "." {
        return Some(canonical_or_existing(path.to_path_buf()));
    }
    if client_cache_manifest_path(path).is_some_and(|manifest_path| manifest_path.is_file()) {
        return Some(canonical_or_existing(path.to_path_buf()));
    }
    language_project_marker_root(language_id, path)
        .filter(|root| root == &canonical_or_existing(path.to_path_buf()))
}

fn is_search_view_arg(args: &[String], index: usize) -> bool {
    if index == 1 && args.first().map(String::as_str) == Some("search") {
        return true;
    }
    index == 0 && args.first().is_some_and(|arg| is_search_view_name(arg))
}

fn is_search_view_name(arg: &str) -> bool {
    matches!(
        arg,
        "api"
            | "callsite"
            | "cfg"
            | "compare"
            | "dependency"
            | "deps"
            | "docs"
            | "docs-use"
            | "features"
            | "lexical"
            | "import"
            | "ingest"
            | "owner"
            | "pattern"
            | "patterns"
            | "policy"
            | "prime"
            | "public-external-types"
            | "query"
            | "semantic-facts"
            | "symbol"
            | "targets"
            | "tests"
            | "workspace"
    )
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
    if path.exists()
        && (project_activation_path(path).is_ok_and(|activation_path| activation_path.is_file())
            || client_cache_manifest_path(path)
                .is_some_and(|manifest_path| manifest_path.is_file()))
    {
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
        "--changed" | "--full" | "--json" | "--receipt-json"
    )
}

fn arg_contains_value(arg: &str) -> bool {
    arg.contains('=')
}

fn option_takes_value(arg: &str) -> bool {
    !matches!(arg, "--changed" | "--full" | "--json" | "--receipt-json")
}

fn invocation_root_is_provider_project(language_id: &str, invocation_root: &Path) -> bool {
    project_activation_path(invocation_root).is_ok_and(|activation_path| activation_path.is_file())
        || language_project_marker_root(language_id, invocation_root).is_some()
}

fn client_cache_manifest_path(project_root: &Path) -> Option<PathBuf> {
    project_state_paths(project_root).ok().map(|paths| {
        paths
            .client_cache_dir
            .join(AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE)
    })
}

fn canonical_or_existing(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}
