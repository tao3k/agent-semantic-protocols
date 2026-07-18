use super::provider_candidates::path_like_tokens;
use super::shell::{command_name, is_separator};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CommandIntent {
    Other,
    DirectRead,
    ContentDump,
    VcsDiffReview,
}

pub(crate) fn command_intent(tokens: &[String]) -> CommandIntent {
    let mut saw_vcs_diff_review = false;
    for stage in command_stages(tokens) {
        match command_stage_intent(stage) {
            CommandIntent::Other => {}
            CommandIntent::VcsDiffReview => saw_vcs_diff_review = true,
            intent => return intent,
        }
    }
    if saw_vcs_diff_review {
        return CommandIntent::VcsDiffReview;
    }
    CommandIntent::Other
}

fn command_stage_intent(tokens: &[String]) -> CommandIntent {
    let command = first_stage_command(tokens);
    if matches!(command.as_deref().map(command_name), Some("read")) {
        return CommandIntent::DirectRead;
    }
    if matches!(command.as_deref().map(command_name), Some("git"))
        && git_diff_outputs_source(tokens)
    {
        return CommandIntent::VcsDiffReview;
    }
    if matches!(
        command.as_deref().map(command_name),
        Some("cat" | "sed" | "nl" | "bat" | "head" | "tail" | "awk" | "less")
    ) && !path_like_tokens(tokens).is_empty()
    {
        return CommandIntent::ContentDump;
    }
    if interpreter_outputs_source(command.as_deref(), tokens) {
        return CommandIntent::ContentDump;
    }
    CommandIntent::Other
}

fn command_stages(tokens: &[String]) -> Vec<&[String]> {
    let mut stages = Vec::new();
    let mut start = 0;
    for (index, token) in tokens.iter().enumerate() {
        if !is_separator(token) {
            continue;
        }
        if start < index {
            stages.push(&tokens[start..index]);
        }
        start = index + 1;
    }
    if start < tokens.len() {
        stages.push(&tokens[start..]);
    }
    stages
}

fn interpreter_outputs_source(command: Option<&str>, tokens: &[String]) -> bool {
    let Some(command) = command.map(command_name) else {
        return false;
    };
    let has_source_path = !path_like_tokens(tokens).is_empty();
    if !has_source_path {
        return false;
    }
    match command {
        "python" | "python3" | "node" | "ruby" => tokens.iter().any(|token| {
            let token = token.as_str();
            token.contains("read_text")
                || token.contains("readFileSync")
                || token.contains("read_file")
                || token.contains(".read(")
                || token.contains("open(")
                || token.contains("File.read")
        }),
        "perl" => tokens.iter().any(|token| {
            perl_reads_files(token) || token.contains("open(") || token.contains("readline")
        }),
        _ => false,
    }
}

fn perl_reads_files(token: &str) -> bool {
    token.starts_with('-')
        && !token.starts_with("--")
        && token
            .chars()
            .skip(1)
            .any(|character| matches!(character, 'n' | 'p'))
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
    for token in iter.by_ref() {
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
