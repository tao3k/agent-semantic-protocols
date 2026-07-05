//! Argument parsing for ASP-owned fast search commands.

use std::path::PathBuf;

use super::search_pipe_source::{SourceSpec, parse_source_spec};
use super::search_pipe_surfaces::{default_search_surfaces, parse_search_surfaces};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SearchPipeArgs {
    pub(super) seed_query: String,
    pub(super) selector: Option<String>,
    pub(super) source: SourceSpec,
    pub(super) workspace: Option<PathBuf>,
    pub(super) scopes: Vec<PathBuf>,
    pub(super) view: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SearchLexicalArgs {
    pub(super) query: String,
    pub(super) pipes: Vec<String>,
    pub(super) owners: Vec<PathBuf>,
    pub(super) workspace: Option<PathBuf>,
    pub(super) view: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OwnerQueryArgs {
    pub(super) owner: PathBuf,
    pub(super) query: String,
    pub(super) view: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OwnerOnlyArgs {
    pub(super) owner: PathBuf,
    pub(super) view: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct IngestArgs {
    pub(super) pipes: Vec<String>,
    pub(super) view: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct FailureArgs {
    pub(super) message: Option<String>,
    pub(super) from_last_check: bool,
    pub(super) view: String,
}

pub(super) fn parse_search_pipe_args(args: &[String]) -> Result<SearchPipeArgs, String> {
    let mut seed_query = args
        .get(2)
        .filter(|seed_query| !seed_query.starts_with('-'))
        .cloned();
    let mut selector = None;
    let mut source = SourceSpec::Auto;
    let mut workspace = None;
    let mut scopes = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = if seed_query.is_some() { 3 } else { 2 };
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                seed_query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--selector" => {
                selector = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--selector requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--owners" | "--owner" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{} requires a value", args[index]))?;
                scopes.extend(split_csv(value).into_iter().map(PathBuf::from));
                index += 2;
            }
            "--packages" | "--package" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{} requires a value", args[index]))?;
                if value.starts_with('-') {
                    return Err(format!("{} requires a value", args[index]));
                }
                scopes.extend(split_csv(value).into_iter().map(PathBuf::from));
                index += 2;
            }
            "--source" => {
                source = parse_source_spec(
                    args.get(index + 1)
                        .ok_or_else(|| "--source requires a value".to_string())?,
                )?;
                index += 2;
            }
            "--workspace" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--workspace requires a project root".to_string())?;
                if value.starts_with('-') {
                    return Err("--workspace requires a project root".to_string());
                }
                let workspace_path = PathBuf::from(value);
                if workspace_path.is_file() {
                    return Err("--workspace requires a directory project root; Keep the file path as the owner/selector".to_string());
                }
                workspace = Some(workspace_path);
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search pipe option: {value}"));
            }
            value => {
                scopes.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    if view == "commands" {
        return Err(
            "search pipe --view commands moved to search suggest --view commands".to_string(),
        );
    }
    if !matches!(view.as_str(), "seeds" | "graph-turbo-request") {
        return Err("search pipe supports --view seeds or --view graph-turbo-request".to_string());
    }
    let seed_query = seed_query.ok_or_else(|| {
        if selector.is_some() {
            "search pipe --selector requires --query or a positional seed query".to_string()
        } else {
            "search pipe requires a seed query".to_string()
        }
    })?;
    if selector.is_none() && seed_query_looks_like_cli_command(&seed_query) {
        return Err(
            "search pipe is a refinement/combinator surface, not a CLI-command lexical search; use search lexical for command terms or search suggest --view commands".to_string(),
        );
    }
    Ok(SearchPipeArgs {
        seed_query,
        selector,
        source,
        workspace,
        scopes,
        view,
    })
}

pub(super) fn parse_owner_query_args(args: &[String]) -> Result<OwnerQueryArgs, String> {
    let mut owner = None;
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--owner" => {
                owner = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--owner requires a value".to_string())?,
                ));
                index += 2;
            }
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown search reasoning owner-query option: {value}"
                ));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerQueryArgs {
        owner: owner.ok_or_else(|| "search reasoning owner-query requires --owner".to_string())?,
        query: query.ok_or_else(|| "search reasoning owner-query requires --query".to_string())?,
        view,
    })
}

pub(super) fn parse_owner_only_args(
    args: &[String],
    profile: &str,
) -> Result<OwnerOnlyArgs, String> {
    let mut owner = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--owner" => {
                owner = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--owner requires a value".to_string())?,
                ));
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown search reasoning {profile} option: {value}"
                ));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerOnlyArgs {
        owner: owner.ok_or_else(|| format!("search reasoning {profile} requires --owner"))?,
        view,
    })
}

pub(super) fn parse_search_owner_items_query_args(
    args: &[String],
) -> Result<OwnerQueryArgs, String> {
    let owner = args
        .get(2)
        .filter(|owner| !owner.starts_with('-'))
        .ok_or_else(|| "search owner requires an owner path".to_string())?;
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerQueryArgs {
        owner: PathBuf::from(owner),
        query: query.unwrap_or_default(),
        view,
    })
}

pub(super) fn parse_ingest_args(args: &[String]) -> Result<IngestArgs, String> {
    let mut pipes = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search ingest option: {value}"));
            }
            "owner" => {
                pipes.push("items".to_string());
                index += 1;
            }
            value => {
                pipes.push(value.to_string());
                index += 1;
            }
        }
    }
    if pipes.is_empty() {
        pipes.extend(default_search_surfaces());
    }
    Ok(IngestArgs { pipes, view })
}

pub(super) fn parse_lexical_args(args: &[String]) -> Result<SearchLexicalArgs, String> {
    let query = args
        .get(2)
        .filter(|query| !query.starts_with('-'))
        .ok_or_else(|| "search lexical requires a query".to_string())?
        .clone();
    let mut pipes = Vec::new();
    let mut owners = Vec::new();
    let mut workspace = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            "--surface" | "--surfaces" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{} requires a value", args[index]))?;
                pipes.extend(parse_search_surfaces(value)?);
                index += 2;
            }
            "--workspace" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--workspace requires a value".to_string())?;
                if workspace.is_some() {
                    return Err("expected at most one --workspace argument".to_string());
                }
                workspace = Some(PathBuf::from(value));
                index += 2;
            }
            "--query-set" | "--owner" | "--dependency" => {
                return Err(format!("search lexical does not support {}", args[index]));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search lexical option: {value}"));
            }
            "owner" => {
                pipes.push("items".to_string());
                index += 1;
            }
            value if matches!(value, "items" | "tests" | "deps" | "dependencies") => {
                pipes.push(value.to_string());
                index += 1;
            }
            value => {
                owners.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    let has_dependency_surface = pipes
        .iter()
        .any(|pipe| matches!(pipe.as_str(), "deps" | "dependencies"));
    let has_owner_surface = pipes.iter().any(|pipe| pipe == "items");
    if has_dependency_surface && has_owner_surface {
        return Err(
            "search lexical does not support combining deps with owner/items; run deps and owner searches separately".to_string(),
        );
    }
    if pipes.is_empty() {
        pipes.extend(default_search_surfaces());
    }
    Ok(SearchLexicalArgs {
        query,
        pipes,
        owners,
        workspace,
        view,
    })
}

pub(super) fn parse_failure_args(args: &[String]) -> Result<FailureArgs, String> {
    let mut message = None;
    let mut positional = Vec::new();
    let mut from_last_check = false;
    let mut view = "seeds".to_string();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--from-last-check" => {
                from_last_check = true;
                index += 1;
            }
            "--message" => {
                message = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--message requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search failure option: {value}"));
            }
            "." => {
                index += 1;
            }
            value => {
                positional.push(value.to_string());
                index += 1;
            }
        }
    }
    if message.is_none() && !positional.is_empty() {
        message = Some(positional.join(" "));
    }
    if from_last_check && message.is_some() {
        return Err(
            "search failure accepts either --from-last-check or failure text, not both".to_string(),
        );
    }
    if !from_last_check && message.is_none() {
        return Err("search failure requires --message or --from-last-check".to_string());
    }
    Ok(FailureArgs {
        message,
        from_last_check,
        view,
    })
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn seed_query_looks_like_cli_command(query: &str) -> bool {
    let tokens = query.split_whitespace().collect::<Vec<_>>();
    if tokens.first().is_some_and(|token| *token == "asp") {
        return true;
    }
    tokens.windows(2).any(|window| {
        matches!(
            window,
            ["search", "pipe"]
                | ["search", "lexical"]
                | ["cache", "status"]
                | ["source-index", "refresh"]
        )
    })
}
