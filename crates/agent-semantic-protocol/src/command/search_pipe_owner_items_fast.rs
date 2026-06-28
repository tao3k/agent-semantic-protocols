//! ASP-owned fast path for `search owner <path> items`.

use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin};
use super::search_config::AspConfig;
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_gerbil_owner_items::run_inline_gerbil_owner_items_query;
use super::search_pipe_owner_query::render_owner_query_frontier;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;

pub(super) struct SearchOwnerItemsFastContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

struct OwnerItemsSearchState<'a> {
    args: &'a [String],
    language_id: &'a str,
    project_root: &'a Path,
    locator_root: &'a Path,
    owner_project_root: PathBuf,
    cache_home: &'a Path,
    config: &'a AspConfig,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    owner: &'a Path,
    query: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerItemsSearchStep {
    Handled,
    Unsupported,
}

impl<'a> OwnerItemsSearchState<'a> {
    fn new(
        args: &'a [String],
        context: SearchOwnerItemsFastContext<'a>,
        owner: &'a Path,
        query: &'a str,
    ) -> Self {
        let owner_project_root = search_workspace_root(
            context.project_root,
            context.locator_root,
            search_owner_items_workspace(args).as_deref(),
        );
        Self {
            args,
            language_id: context.language_id,
            project_root: context.project_root,
            locator_root: context.locator_root,
            owner_project_root,
            cache_home: context.cache_home,
            config: context.config,
            provider_context: context.provider_context,
            owner,
            query,
        }
    }

    fn provider_required_error(&self) -> String {
        format!(
            "python search owner items requires provider-owned owner-items; owner=`{}`; no Rust inline fallback executed",
            self.owner.display()
        )
    }

    fn try_inline_gerbil(&self) -> Result<OwnerItemsSearchStep, String> {
        if run_inline_gerbil_owner_items_query(
            self.language_id,
            self.owner,
            self.query,
            &self.owner_project_root,
        )? {
            return Ok(OwnerItemsSearchStep::Handled);
        }
        Ok(OwnerItemsSearchStep::Unsupported)
    }

    fn try_provider(&self) -> Result<OwnerItemsSearchStep, String> {
        run_provider_owner_items_query(
            self.language_id,
            self.args,
            self.owner,
            &self.owner_project_root,
            self.cache_home,
            self.config,
            self.provider_context,
        )
    }

    fn render_native_owner_query(&self) -> String {
        render_owner_query_frontier(
            self.language_id,
            self.project_root,
            self.locator_root,
            self.owner,
            self.query,
        )
    }
}

pub(super) fn run_search_owner_items_query_command(
    args: &[String],
    context: SearchOwnerItemsFastContext<'_>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(context.frontier_receipt)?;
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if !matches!(owner_query_args.view.as_str(), "seeds" | "hits") {
        return Err(
            "search owner items fast path supports --view seeds or --view hits".to_string(),
        );
    }
    let state = OwnerItemsSearchState::new(
        args,
        context,
        &owner_query_args.owner,
        &owner_query_args.query,
    );
    if state.language_id == "python" {
        if state.try_provider()? == OwnerItemsSearchStep::Handled {
            return Ok(());
        }
        return Err(state.provider_required_error());
    }
    if state.try_inline_gerbil()? == OwnerItemsSearchStep::Handled {
        return Ok(());
    }
    if state.try_provider()? == OwnerItemsSearchStep::Handled {
        return Ok(());
    }
    print!("{}", state.render_native_owner_query());
    Ok(())
}

fn search_owner_items_workspace(args: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--workspace" {
            return args.get(index + 1).map(PathBuf::from);
        }
        index += 1;
    }
    None
}

fn run_provider_owner_items_query(
    language_id: &str,
    args: &[String],
    owner: &Path,
    project_root: &Path,
    cache_home: &Path,
    config: &AspConfig,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<OwnerItemsSearchStep, String> {
    let Some(context) = provider_context else {
        return Ok(OwnerItemsSearchStep::Unsupported);
    };
    if !context.provider.search_capabilities.owner_items {
        return Ok(OwnerItemsSearchStep::Unsupported);
    }
    let existing_owner_path = owner_path_exists(project_root, owner);
    let invocation = provider_invocation_with_profile(
        context.profiles,
        context.provider,
        args,
        project_root,
        config,
    )?;
    let output = run_provider_command_with_stdin(
        language_id,
        context.provider,
        &invocation,
        project_root,
        cache_home,
        Vec::new(),
    )?;
    if !output.status.success() {
        if !existing_owner_path {
            return Ok(OwnerItemsSearchStep::Unsupported);
        }
        return Err(provider_owner_items_failure(
            "provider-owned owner-items failed",
            owner,
            output.stderr.as_ref(),
            existing_owner_path,
        ));
    }
    if output
        .stdout
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == 0)
    {
        if !existing_owner_path {
            return Ok(OwnerItemsSearchStep::Unsupported);
        }
        return Err(provider_owner_items_failure(
            "provider-owned owner-items produced empty output",
            owner,
            output.stderr.as_ref(),
            existing_owner_path,
        ));
    }
    io::stderr()
        .write_all(output.stderr.as_ref())
        .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    io::stdout()
        .write_all(output.stdout.as_ref())
        .map_err(|error| format!("failed to write provider stdout: {error}"))?;
    Ok(OwnerItemsSearchStep::Handled)
}

fn owner_path_exists(project_root: &Path, owner: &Path) -> bool {
    let path = if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    };
    fs::metadata(path).is_ok()
}

fn provider_owner_items_failure(
    message: &str,
    owner: &Path,
    stderr: &[u8],
    existing_owner_path: bool,
) -> String {
    let owner_state = if existing_owner_path {
        "existing owner path"
    } else {
        "owner"
    };
    let mut failure = format!(
        "{message} for {owner_state} `{}`; no fallback executed",
        owner.display()
    );
    let provider_stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !provider_stderr.is_empty() {
        failure.push_str(": ");
        failure.push_str(&provider_stderr);
    }
    failure
}

fn search_workspace_root(
    project_root: &Path,
    locator_root: &Path,
    explicit_workspace: Option<&Path>,
) -> PathBuf {
    let Some(workspace) = explicit_workspace else {
        return project_root.to_path_buf();
    };
    let workspace = if workspace.is_absolute() {
        workspace.to_path_buf()
    } else {
        locator_root.join(workspace)
    };
    normalize_path(&workspace)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
