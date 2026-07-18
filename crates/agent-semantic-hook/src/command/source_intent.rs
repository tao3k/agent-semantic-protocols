//! Public source-command classification backed by the hook command matcher.

/// Source-access intent produced by the same parser-owned matcher used by the
/// normal hook classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCommandIntent {
    Other,
    DirectRead,
    ContentDump,
    VcsDiffReview,
}

/// Classify a raw shell command through the shared hook command matcher.
pub fn classify_source_command_intent(command: &str) -> SourceCommandIntent {
    match super::intent::command_intent(&super::shell::semantic_shell_tokens(command)) {
        super::intent::CommandIntent::Other => SourceCommandIntent::Other,
        super::intent::CommandIntent::DirectRead => SourceCommandIntent::DirectRead,
        super::intent::CommandIntent::ContentDump => SourceCommandIntent::ContentDump,
        super::intent::CommandIntent::VcsDiffReview => SourceCommandIntent::VcsDiffReview,
    }
}
