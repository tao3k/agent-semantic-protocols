use super::raw_search;
use super::shell::{command_name, is_separator};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CommandIntent {
    Other,
    DirectRead,
    ContentDump,
    RawSearch,
}

pub(crate) fn command_intent(tokens: &[String]) -> CommandIntent {
    let command = first_stage_command(tokens);
    if matches!(command.as_deref().map(command_name), Some("read")) {
        return CommandIntent::DirectRead;
    }
    if matches!(command.as_deref().map(command_name), Some("git"))
        && git_diff_outputs_source(tokens)
    {
        return CommandIntent::ContentDump;
    }
    if matches!(
        command.as_deref().map(command_name),
        Some("cat" | "sed" | "nl" | "bat" | "head" | "tail" | "awk" | "less")
    ) {
        return CommandIntent::ContentDump;
    }
    if raw_search::raw_search_stage(tokens).is_some() {
        return CommandIntent::RawSearch;
    }
    CommandIntent::Other
}

fn git_diff_outputs_source(tokens: &[String]) -> bool {
    if git_subcommand(tokens) != Some("diff") {
        return false;
    }
    !tokens
        .iter()
        .any(|token| git_diff_metadata_only_flag(token.as_str()))
}

fn git_subcommand(tokens: &[String]) -> Option<&str> {
    let mut iter = tokens.iter().map(String::as_str);
    while let Some(token) = iter.next() {
        if is_separator(token) {
            continue;
        }
        if command_name(token) != "git" {
            return None;
        }
        break;
    }
    while let Some(token) = iter.next() {
        if is_separator(token) {
            continue;
        }
        if git_global_option_consumes_value(token) {
            iter.next();
            continue;
        }
        if git_global_option_with_inline_value(token) || token.starts_with('-') {
            continue;
        }
        return Some(command_name(token));
    }
    None
}

fn git_global_option_consumes_value(token: &str) -> bool {
    matches!(
        token,
        "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env"
    )
}

fn git_global_option_with_inline_value(token: &str) -> bool {
    token.starts_with("--git-dir=")
        || token.starts_with("--work-tree=")
        || token.starts_with("--namespace=")
        || token.starts_with("--config-env=")
}

fn git_diff_metadata_only_flag(token: &str) -> bool {
    matches!(
        token,
        "--stat"
            | "--shortstat"
            | "--numstat"
            | "--dirstat"
            | "--summary"
            | "--name-only"
            | "--name-status"
            | "--check"
            | "--quiet"
            | "--exit-code"
    ) || token.starts_with("--stat=")
        || token.starts_with("--dirstat=")
}

fn first_stage_command(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find(|token| !token.starts_with('-') && !is_separator(token))
        .cloned()
}
