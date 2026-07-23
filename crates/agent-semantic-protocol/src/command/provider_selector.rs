use agent_semantic_hook::HookRuntime;

fn registered_language_facades() -> Vec<String> {
    agent_semantic_hook::registered_language_ids()
}

pub(super) fn registered_language_facades_line() -> String {
    registered_language_facades().join("|")
}

pub(crate) fn is_language_facade(language_id: &str) -> bool {
    registered_language_facades()
        .iter()
        .any(|facade| facade == language_id)
}

pub(crate) fn unsupported_language_facade_message(
    requested_facade: &str,
    command: Option<&str>,
    runtime: Option<&HookRuntime>,
) -> String {
    let known_facades = registered_language_facades_line();
    let active_facades = runtime
        .map(active_language_facades)
        .filter(|facades| !facades.is_empty());
    let suggested_facade = runtime.and_then(|runtime| {
        suggested_language_facade_for_request(requested_facade, &active_language_facades(runtime))
    });
    let mut lines = vec![
        format!("unsupported ASP language facade `{requested_facade}`."),
        "ASP facades are language IDs, not package or library names.".to_string(),
        format!("Known language facades: {known_facades}."),
    ];
    if let Some(active_facades) = active_facades.as_deref() {
        lines.push(format!("Active language facades: {active_facades}."));
    }
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        lines.push(format!("Suggested matching facade: {suggested_facade}."));
    }
    lines.extend([String::new(), "## Run Next".to_string()]);
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        let command = command.unwrap_or("guide");
        lines.push(format!("asp {suggested_facade} {command} ..."));
    } else {
        lines.extend([
            "asp providers".to_string(),
            "asp fd -query '<path-or-language-term>' '.'".to_string(),
            "asp rg -query '<feature-term>' '<bounded-scope>'".to_string(),
        ]);
    }
    lines.extend([
        String::new(),
        "## Rules".to_string(),
        "Only run `asp <language> search|query` when the facade is listed and matches the target language.".to_string(),
        "Do not switch to an unrelated active facade just because it is the only provider in this repository.".to_string(),
        "For unsupported target-language files, use provider-neutral finder commands or install/activate a matching provider.".to_string(),
    ]);
    lines.join("\n")
}

fn active_language_facades(runtime: &HookRuntime) -> String {
    runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}

fn suggested_language_facade_for_request(
    requested_facade: &str,
    active_facades: &str,
) -> Option<String> {
    if requested_facade.eq_ignore_ascii_case("effect")
        && active_facades
            .split('|')
            .any(|facade| facade == "typescript")
    {
        return Some("typescript".to_string());
    }
    None
}
pub(super) fn reject_search_file_workspace(
    args: &[String],
    invocation_root: &Path,
) -> Result<(), String> {
    if !matches!(args.first().map(String::as_str), Some("search")) {
        return Ok(());
    }
    let Some(workspace) = option_value(args, "--workspace") else {
        return Ok(());
    };
    if workspace.starts_with('-') {
        return Ok(());
    }
    let workspace_path = PathBuf::from(workspace);
    let workspace_path = if workspace_path.is_absolute() {
        workspace_path
    } else {
        invocation_root.join(workspace_path)
    };
    if workspace_path.is_file() {
        return Err(
            "--workspace requires a directory project root; Keep the file path as the owner/selector"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn invalid_source_selector_query_message(
    language_id: &str,
    selector: &str,
    args: &[String],
) -> String {
    let workspace = option_value(args, "--workspace").unwrap_or(".");
    format!(
        "invalid query selector `{selector}`: file selectors are not executable query selectors; query an exact parser-owned item selector such as {language_id}://path#item/function/name; recover with search owner <path> items\nselectorState=file-selector\nprojection=query\nallowed=false\nreason=file-selectors-are-not-query-selectors\nnextAction=materialize-owner-items\nnextCommand=asp {language_id} search owner {selector} items --workspace {workspace} --view seeds\nrequiredSelector={language_id}://{selector}#item/<kind>/<name>"
    )
}

pub(super) fn is_plain_file_selector_code_query(args: &[String]) -> bool {
    if !matches!(args.first().map(String::as_str), Some("query"))
        || !args.iter().any(|arg| arg == "--code")
        || args.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "--json" | "--term" | "--treesitter-query" | "--names-only"
            )
        })
        || args
            .iter()
            .any(|arg| arg == "--from-hook" || arg.starts_with("--from-hook="))
    {
        return false;
    }
    let Some(selector) = option_value(args, "--selector") else {
        return false;
    };
    !selector.contains("://") && selector.split_once(':').is_none()
}

pub(super) fn is_file_selector_query(args: &[String]) -> bool {
    if !matches!(args.first().map(String::as_str), Some("query"))
        || args
            .iter()
            .any(|arg| arg == "--term" || arg == "--treesitter-query")
    {
        return false;
    }
    let Some(selector) = option_value(args, "--selector") else {
        return false;
    };
    !selector.contains("://") && selector.split_once(':').is_none()
}

pub(super) fn is_provider_owned_structural_selector_query(
    language_id: &str,
    args: &[String],
) -> bool {
    if !matches!(args.first().map(String::as_str), Some("query"))
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--term" | "--treesitter-query"))
    {
        return false;
    }
    let Some(selector) = option_value(args, "--selector") else {
        return false;
    };
    selector
        .strip_prefix(language_id)
        .is_some_and(|suffix| suffix.starts_with("://") && suffix.contains("#item/"))
}

pub(super) fn provider_owned_structural_owner_path<'a>(
    language_id: &str,
    args: &'a [String],
) -> Option<&'a str> {
    if !is_provider_owned_structural_selector_query(language_id, args) {
        return None;
    }
    let selector_prefix = match language_id {
        "rust" => "rust://",
        "typescript" => "typescript://",
        _ => return None,
    };
    option_value(args, "--selector")?
        .strip_prefix(selector_prefix)?
        .split_once('#')
        .map(|(owner_path, _)| owner_path)
}

pub(super) fn provider_owned_structural_selector<'a>(
    language_id: &str,
    args: &'a [String],
) -> Option<&'a str> {
    if !is_provider_owned_structural_selector_query(language_id, args) {
        return None;
    }
    option_value(args, "--selector")
}

pub(super) fn reject_registered_source_selector_query(
    language_id: &str,
    args: &[String],
    provider: &agent_semantic_hook::ActivatedProvider,
) -> Result<(), String> {
    if !is_file_selector_query(args) {
        return Ok(());
    }
    let Some(selector) = option_value(args, "--selector") else {
        return Ok(());
    };
    let selector_path = selector
        .split_once(':')
        .map_or(selector, |(path, _range)| path);
    let Some(extension) = Path::new(selector_path)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return Ok(());
    };
    let registered_source = provider.source_extensions.iter().any(|source| {
        source
            .trim_start_matches('.')
            .eq_ignore_ascii_case(extension)
    });
    if !registered_source {
        return Ok(());
    }
    Err(invalid_source_selector_query_message(
        language_id,
        selector,
        args,
    ))
}

pub(super) fn reject_manifest_source_selector_query_code(
    language_id: &str,
    args: &[String],
) -> Result<(), String> {
    if !is_plain_file_selector_code_query(args) {
        return Ok(());
    }
    let Some(selector) = option_value(args, "--selector") else {
        return Ok(());
    };
    let selector_path = selector
        .split_once(':')
        .map_or(selector, |(path, _range)| path);
    let Some(extension) = Path::new(selector_path)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return Ok(());
    };
    let registered_source = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == language_id)
        .is_some_and(|manifest| {
            manifest.source.default_extensions.iter().any(|source| {
                source
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(extension)
            })
        });
    if !registered_source {
        return Ok(());
    }
    Err(invalid_source_selector_query_message(
        language_id,
        selector,
        args,
    ))
}
use std::path::{Path, PathBuf};
pub(super) fn option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}
