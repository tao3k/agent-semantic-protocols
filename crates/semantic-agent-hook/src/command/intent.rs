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

fn first_stage_command(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find(|token| !token.starts_with('-') && !is_separator(token))
        .cloned()
}
