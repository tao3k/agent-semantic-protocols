//! Bounded command-stage and prefix matching contracts.

use crate::bash_parser;

/// Maximum tokens inspected in one parsed command stage.
pub const MAX_STAGE_TOKENS: usize = 256;
/// Maximum normalized command candidates inspected per match.
pub const MAX_COMMAND_CANDIDATES: usize = 32;

/// Return the basename used to compare executable tokens across absolute paths.
pub(crate) fn command_token_basename(token: &str) -> &str {
    token.rsplit(['/', '\\']).next().unwrap_or(token)
}

/// Bounded prefix-match outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
    Matched,
    NotMatched,
    BudgetExceeded,
}

impl PrefixMatch {
    /// Budget exhaustion is protected routing, never an escape hatch.
    pub const fn routes_protected(self) -> bool {
        !matches!(self, Self::NotMatched)
    }
}

/// One parser-owned normalized shell command stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStageV1 {
    pub(crate) words: Vec<String>,
}

impl CommandStageV1 {
    /// Construct a stage from normalized words.
    pub fn new(words: Vec<String>) -> Self {
        Self { words }
    }

    /// Borrow the normalized words.
    pub fn words(&self) -> &[String] {
        &self.words
    }

    /// Executable word for this parsed shell stage.
    pub fn executable(&self) -> Option<&str> {
        self.words.first().map(String::as_str)
    }
}

/// Match normalized command stages against a configured argv prefix.
pub fn command_stages_match_prefix(stages: &[CommandStageV1], prefix: &[String]) -> PrefixMatch {
    command_stages_match_prefix_impl(stages, prefix)
}

fn command_stages_match_prefix_impl(stages: &[CommandStageV1], prefix: &[String]) -> PrefixMatch {
    if prefix.is_empty() {
        return PrefixMatch::Matched;
    }

    let mut inspected_candidates = 0usize;
    for stage in stages {
        let words = stage.words();
        if words.len() > MAX_STAGE_TOKENS {
            return PrefixMatch::BudgetExceeded;
        }
        if words.len() < prefix.len() {
            continue;
        }
        if inspected_candidates == MAX_COMMAND_CANDIDATES {
            return PrefixMatch::BudgetExceeded;
        }
        inspected_candidates += 1;
        if candidate_matches_prefix(words, prefix) {
            return PrefixMatch::Matched;
        }
    }
    PrefixMatch::NotMatched
}

/// Match one normalized candidate against an argv prefix.
pub fn candidate_matches_prefix(candidate: &[String], prefix: &[String]) -> bool {
    candidate.len() >= prefix.len()
        && candidate
            .iter()
            .zip(prefix)
            .enumerate()
            .all(|(index, (actual, expected))| {
                actual.eq_ignore_ascii_case(expected)
                    || (index == 0 && command_token_basename(actual).eq_ignore_ascii_case(expected))
            })
}

/// Parse a Bash command into bounded normalized command candidates.
pub fn parse_bash_command_candidates(command: &str) -> Result<Vec<CommandStageV1>, String> {
    parse_bash_command_candidates_impl(command)
}

fn parse_bash_command_candidates_impl(command: &str) -> Result<Vec<CommandStageV1>, String> {
    let tokens = bash_parser::bash_ast_tokens(command)
        .ok_or_else(|| "bash-tree-sitter-parse-failed".to_string())?;
    let raw_stages = bash_parser::split_command_stages(tokens);
    let mut pending = std::collections::VecDeque::from(raw_stages);
    let mut candidates = Vec::new();
    while let Some(words) = pending.pop_front() {
        if words.is_empty() {
            continue;
        }
        let is_separator_stage = words.len() == 1 && bash_parser::is_separator(words[0].as_str());
        if !is_separator_stage
            && candidates
                .iter()
                .any(|candidate: &CommandStageV1| candidate.words == words)
        {
            continue;
        }
        let normalized = bash_parser::unwrap_command_stage(&words)?;
        candidates.push(CommandStageV1 { words });
        for stage in normalized {
            if !stage.is_empty() {
                pending.push_back(stage);
            }
        }
    }
    (!candidates.is_empty())
        .then_some(candidates)
        .ok_or_else(|| "bash-tree-sitter-empty-command".to_string())
}

/// Public Bash prefix-match result including fail-closed parser errors.
#[derive(Debug, PartialEq)]
pub enum BashCommandMatchV1 {
    Parsed(PrefixMatch),
    InvalidSyntax { reason: &'static str },
}

/// Parse and match a Bash command against a string-slice argv prefix.
pub fn match_bash_command_prefix(command: &str, prefix: &[&str]) -> BashCommandMatchV1 {
    match parse_bash_command_candidates(command) {
        Ok(stages) => {
            let prefix = prefix
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();
            BashCommandMatchV1::Parsed(command_stages_match_prefix(&stages, &prefix))
        }
        Err(_) => BashCommandMatchV1::InvalidSyntax {
            reason: "bash-tree-sitter-parse-failed",
        },
    }
}
