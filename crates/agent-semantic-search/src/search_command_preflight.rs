//! Search command validation and local pre-provider hot-path budgets.
//!
//! This module owns language-neutral `asp <language> search ...` preflight
//! semantics before command-layer dispatch can invoke a provider process.

use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPreflightLanguageId(String);

impl SearchPreflightLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SearchPreflightLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SearchPreflightLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Result of applying search command preflight to raw CLI arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchCommandPreflightOutcome {
    /// The argument vector is not owned by this preflight route.
    NotApplicable,
    /// The command shape is owned by preflight and passed validation.
    Passed,
    /// The command failed closed with an agent-facing diagnostic.
    Rejected(String),
}

/// Search command preflight request passed from thin CLI command facades.
pub struct SearchCommandPreflightRequest<'a> {
    language_id: &'a SearchPreflightLanguageId,
    workspace: Option<&'a Path>,
    project_root: &'a Path,
    command: SearchCommandPreflightCommand<'a>,
}

/// Provider-declared language facts used to admit an owner-item request.
pub struct OwnerItemsLanguageAdmission<'a> {
    expected_extensions: &'a [String],
    suggested_language: Option<&'a str>,
}

impl<'a> OwnerItemsLanguageAdmission<'a> {
    /// Build an admission request from the activated provider registry.
    pub fn new(expected_extensions: &'a [String], suggested_language: Option<&'a str>) -> Self {
        Self {
            expected_extensions,
            suggested_language,
        }
    }
}

impl<'a> SearchCommandPreflightRequest<'a> {
    /// Build a preflight request for `search owner <owner> items`.
    pub fn owner_items(
        language_id: &'a SearchPreflightLanguageId,
        owner: &'a Path,
        workspace: Option<&'a Path>,
        project_root: &'a Path,
    ) -> Self {
        Self {
            language_id,
            workspace,
            project_root,
            command: SearchCommandPreflightCommand::OwnerItems { owner },
        }
    }
}

enum SearchCommandPreflightCommand<'a> {
    OwnerItems { owner: &'a Path },
}

/// Local elapsed-time budget for search command preflight.
pub struct SearchCommandPreflightBudget {
    max_elapsed: Duration,
}

impl SearchCommandPreflightBudget {
    /// Create a preflight budget with a maximum local elapsed duration.
    pub fn new(max_elapsed: Duration) -> Self {
        Self { max_elapsed }
    }
}

/// Validate a search command with the default local hot-path budget.
pub fn preflight_search_command(request: SearchCommandPreflightRequest<'_>) -> Result<(), String> {
    preflight_search_command_with_budget(
        request,
        SearchCommandPreflightBudget::new(Duration::from_millis(20)),
    )
}

/// Validate raw `asp <language> search ...` arguments when a preflight route applies.
pub fn preflight_search_command_args(
    language_id: &SearchPreflightLanguageId,
    args: &[String],
    project_root: &Path,
) -> SearchCommandPreflightOutcome {
    preflight_search_command_args_with_admission(language_id, args, project_root, None)
}

/// Validate raw owner-item arguments against the activated provider's source contract.
pub fn preflight_search_command_args_with_owner_language_admission(
    language_id: &SearchPreflightLanguageId,
    args: &[String],
    project_root: &Path,
    admission: OwnerItemsLanguageAdmission<'_>,
) -> SearchCommandPreflightOutcome {
    preflight_search_command_args_with_admission(language_id, args, project_root, Some(admission))
}

fn preflight_search_command_args_with_admission(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    admission: Option<OwnerItemsLanguageAdmission<'_>>,
) -> SearchCommandPreflightOutcome {
    if !is_search_owner_query(args) {
        return SearchCommandPreflightOutcome::NotApplicable;
    }
    let workspace = search_workspace_arg(args);
    match validate_search_owner_query_shape(args) {
        Ok(owner) => {
            match preflight_search_command(SearchCommandPreflightRequest::owner_items(
                language_id,
                owner,
                workspace.as_deref(),
                project_root,
            )) {
                Ok(()) => match admission {
                    Some(admission) => match preflight_owner_items_language_admission(
                        language_id,
                        owner,
                        workspace.as_deref(),
                        project_root,
                        admission,
                    ) {
                        Ok(()) => SearchCommandPreflightOutcome::Passed,
                        Err(error) => SearchCommandPreflightOutcome::Rejected(error),
                    },
                    None => SearchCommandPreflightOutcome::Passed,
                },
                Err(error) => SearchCommandPreflightOutcome::Rejected(error),
            }
        }
        Err(reason) => {
            SearchCommandPreflightOutcome::Rejected(render_invalid_search_owner_command_error(
                language_id,
                reason,
                workspace.as_deref(),
                project_root,
            ))
        }
    }
}

/// Check whether a provider-declared extension owns a source extension.
pub fn source_extension_is_declared(extension: &str, expected_extensions: &[String]) -> bool {
    expected_extensions.iter().any(|expected| {
        expected
            .trim_start_matches('.')
            .eq_ignore_ascii_case(extension.trim_start_matches('.'))
    })
}

/// Validate raw search arguments using only the invocation root and `--workspace` text.
pub fn preflight_search_command_args_at_invocation_root(
    language_id: &SearchPreflightLanguageId,
    args: &[String],
    invocation_root: &Path,
) -> SearchCommandPreflightOutcome {
    let project_root = preflight_project_root(args, invocation_root);
    preflight_search_command_args(language_id, args, &project_root)
}

/// Validate a search command with an explicit local hot-path budget.
pub fn preflight_search_command_with_budget(
    request: SearchCommandPreflightRequest<'_>,
    budget: SearchCommandPreflightBudget,
) -> Result<(), String> {
    let started_at = Instant::now();
    let result = match request.command {
        SearchCommandPreflightCommand::OwnerItems { owner } => {
            preflight_owner_items_command(&request, owner)
        }
    };
    if let Err(error) = &result {
        return Err(error.clone());
    }
    let elapsed = started_at.elapsed();
    if elapsed > budget.max_elapsed {
        return Err(render_preflight_budget_error(
            request.language_id,
            elapsed,
            budget.max_elapsed,
        ));
    }
    result
}

fn preflight_owner_items_command(
    request: &SearchCommandPreflightRequest<'_>,
    owner: &Path,
) -> Result<(), String> {
    if let Some(reason) = invalid_owner_items_owner_reason(owner, request.project_root) {
        return Err(render_invalid_owner_items_owner_error(
            request.language_id,
            owner,
            request.workspace,
            request.project_root,
            reason,
        ));
    }
    Ok(())
}

fn invalid_owner_items_owner_reason(
    owner: &Path,
    project_root: &Path,
) -> Option<InvalidOwnerReason> {
    let owner_text = owner.to_string_lossy();
    if owner_text.trim().is_empty() {
        return Some(InvalidOwnerReason::EmptyOwner);
    }
    if matches!(owner_text.as_ref(), "." | "./") {
        return Some(InvalidOwnerReason::WorkspaceRootOwner);
    }
    let owner_path = search_owner_source_path(project_root, owner);
    let lexical_owner = lexical_normalize(&owner_path);
    let lexical_root = lexical_normalize(project_root);
    if lexical_owner == lexical_root {
        return Some(InvalidOwnerReason::WorkspaceRootOwner);
    }
    let Ok(link_metadata) = std::fs::symlink_metadata(&owner_path) else {
        return Some(InvalidOwnerReason::MissingOwner);
    };
    let owner_is_symlink = link_metadata.file_type().is_symlink();
    let metadata = if owner_is_symlink {
        let Ok(metadata) = std::fs::metadata(&owner_path) else {
            return Some(InvalidOwnerReason::MissingOwner);
        };
        metadata
    } else {
        link_metadata
    };
    if metadata.is_dir() {
        return Some(InvalidOwnerReason::DirectoryOwner);
    }
    if !lexical_owner.starts_with(&lexical_root) {
        return Some(InvalidOwnerReason::OutsideWorkspace);
    }
    let relative_owner = lexical_owner
        .strip_prefix(&lexical_root)
        .expect("lexical workspace containment checked above");
    let mut cursor = lexical_root.clone();
    let mut components = relative_owner.components().peekable();
    let mut parent_has_symlink = false;
    while let Some(component) = components.next() {
        cursor.push(component);
        if components.peek().is_none() {
            break;
        }
        if std::fs::symlink_metadata(&cursor)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            parent_has_symlink = true;
            break;
        }
    }
    if (owner_is_symlink || parent_has_symlink)
        && let Ok(canonical_owner) = owner_path.canonicalize()
        && !canonical_owner.starts_with(&lexical_root)
    {
        match project_root.canonicalize() {
            Ok(canonical_root) if canonical_owner.starts_with(&canonical_root) => {}
            _ => return Some(InvalidOwnerReason::OutsideWorkspace),
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvalidOwnerReason {
    EmptyOwner,
    WorkspaceRootOwner,
    DirectoryOwner,
    MissingOwner,
    OutsideWorkspace,
}

impl InvalidOwnerReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::EmptyOwner => "empty-owner",
            Self::WorkspaceRootOwner => "workspace-root-owner",
            Self::DirectoryOwner => "directory-owner",
            Self::MissingOwner => "missing-owner",
            Self::OutsideWorkspace => "outside-workspace",
        }
    }
}

fn preflight_owner_items_language_admission(
    language_id: &str,
    owner: &Path,
    workspace: Option<&Path>,
    project_root: &Path,
    admission: OwnerItemsLanguageAdmission<'_>,
) -> Result<(), String> {
    let owner_path = search_owner_source_path(project_root, owner);
    let owner_extension = owner_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("none");
    if source_extension_is_declared(owner_extension, admission.expected_extensions) {
        return Ok(());
    }
    Err(render_owner_language_mismatch_error(
        language_id,
        owner,
        workspace,
        project_root,
        owner_extension,
        admission.expected_extensions,
        admission.suggested_language,
    ))
}

fn render_owner_language_mismatch_error(
    language_id: &str,
    owner: &Path,
    workspace: Option<&Path>,
    project_root: &Path,
    owner_extension: &str,
    expected_extensions: &[String],
    suggested_language: Option<&str>,
) -> String {
    let workspace = diagnostic_workspace(workspace, project_root);
    let mut expected_extensions = expected_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.'))
        .filter(|extension| !extension.is_empty())
        .collect::<Vec<_>>();
    expected_extensions.sort_unstable();
    expected_extensions.dedup();
    let expected_extensions = if expected_extensions.is_empty() {
        "none".to_string()
    } else {
        expected_extensions.join("|")
    };
    let suggested_language_field = suggested_language
        .map(|language| format!(" suggestedLanguage={language}"))
        .unwrap_or_default();
    let next_action = suggested_language.map_or_else(
        || "nextAction=select-language-for-owner-extension".to_string(),
        |language| {
            format!(
                "nextCommand=asp {language} search owner '{}' items --query '<symbol-or-a|b|c>' --workspace {workspace} --view seeds",
                escape_diagnostic_value(&owner.display().to_string()),
            )
        },
    );
    format!(
        "[asp-search-query-error] code=owner-language-mismatch owner=\"{}\" requestedLanguage={language_id} ownerExtension={owner_extension} expectedExtensions={expected_extensions}{suggested_language_field}\n\
|hint owner-item admission rejected an extension not declared by the requested provider; no provider was started\n\
{next_action}",
        escape_diagnostic_value(&owner.display().to_string()),
    )
}

fn search_owner_source_path(project_root: &Path, owner: &Path) -> PathBuf {
    if owner.is_absolute() {
        lexical_normalize(owner)
    } else {
        lexical_normalize(&project_root.join(owner))
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut components = path.components();
    if path.is_absolute() {
        if let Some(Component::Prefix(prefix)) = components.next() {
            normalized.push(prefix.as_os_str());
        } else if let Some(Component::RootDir) = path.components().next() {
            normalized.push(Path::new("/"));
        }
    }

    for component in components {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(component) => normalized.push(component),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    if normalized.as_os_str().is_empty() {
        normalized.push(".");
    }
    normalized
}

fn render_invalid_owner_items_owner_error(
    language_id: &str,
    owner: &Path,
    workspace: Option<&Path>,
    project_root: &Path,
    reason: InvalidOwnerReason,
) -> String {
    let workspace = diagnostic_workspace(workspace, project_root);
    format!(
        "[asp-search-query-error] code=invalid-owner owner=\"{}\" reason={}\n\
|hint search owner requires a concrete source owner path; workspace roots and directories are search scopes, not owners\n\
nextCommand=asp {language_id} search pipe '<focused terms>' --workspace {workspace} --view seeds\n\
example=asp {language_id} search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace {workspace} --view seeds",
        escape_diagnostic_value(&owner.display().to_string()),
        reason.as_str(),
    )
}

fn render_preflight_budget_error(
    language_id: &str,
    elapsed: Duration,
    max_elapsed: Duration,
) -> String {
    format!(
        "[asp-search-query-error] code=preflight-budget-exceeded language={language_id} elapsedMs={} maxMs={}\n\
|hint search command validation exceeded its local budget before provider dispatch\n\
nextCommand=asp {language_id} search pipe '<focused terms>' --workspace <workspace-root> --view seeds",
        elapsed.as_millis(),
        max_elapsed.as_millis(),
    )
}

fn diagnostic_workspace(workspace: Option<&Path>, project_root: &Path) -> String {
    workspace
        .filter(|workspace| workspace.is_absolute())
        .map(|workspace| workspace.display().to_string())
        .unwrap_or_else(|| project_root.display().to_string())
}

fn escape_diagnostic_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn is_search_owner_query(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some("search")
        && args.get(1).map(String::as_str) == Some("owner")
}

enum InvalidSearchOwnerCommand<'a> {
    MissingOwnerPath,
    MissingItemsSurface { owner: &'a str },
}

fn validate_search_owner_query_shape(
    args: &[String],
) -> Result<&Path, InvalidSearchOwnerCommand<'_>> {
    let owner = args
        .get(2)
        .filter(|owner| !owner.starts_with('-'))
        .ok_or(InvalidSearchOwnerCommand::MissingOwnerPath)?;
    if args.get(3).is_some_and(|item| item == "items") {
        return Ok(Path::new(owner));
    }
    Err(InvalidSearchOwnerCommand::MissingItemsSurface { owner })
}

fn render_invalid_search_owner_command_error(
    language_id: &str,
    reason: InvalidSearchOwnerCommand<'_>,
    workspace: Option<&Path>,
    project_root: &Path,
) -> String {
    let workspace = diagnostic_workspace(workspace, project_root);
    let (subject, owner) = match reason {
        InvalidSearchOwnerCommand::MissingOwnerPath => {
            ("reason=missing-owner-path".to_string(), "<owner-path>")
        }
        InvalidSearchOwnerCommand::MissingItemsSurface { owner } => (
            format!(
                "owner=\"{}\" reason=missing-items-surface",
                escape_diagnostic_value(owner)
            ),
            owner,
        ),
    };
    format!(
        "[asp-search-query-error] code=invalid-search-command {subject}\n\
|hint search owner requires the `items` surface: search owner <owner-path> items --query '<terms>'\n\
nextCommand=asp {language_id} search owner {owner} items --query '<symbol-or-a|b|c>' --workspace {workspace} --view seeds",
        owner = escape_diagnostic_value(owner),
        language_id = language_id,
        workspace = workspace,
    )
}

fn search_workspace_arg(args: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--workspace" {
            return args.get(index + 1).map(PathBuf::from);
        }
        if let Some(workspace) = arg.strip_prefix("--workspace=") {
            return Some(PathBuf::from(workspace));
        }
        index += 1;
    }
    None
}

fn preflight_project_root(args: &[String], invocation_root: &Path) -> PathBuf {
    match search_workspace_arg(args) {
        Some(workspace) if workspace.is_absolute() => lexical_normalize(&workspace),
        Some(workspace) => lexical_normalize(&invocation_root.join(workspace)),
        None => invocation_root.to_path_buf(),
    }
}
