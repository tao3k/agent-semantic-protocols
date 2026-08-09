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

/// Match a configured command prefix at any bounded argv position in a parsed stage.
///
/// This is the generic wrapper mode: the outer executable remains parser-visible,
/// while a protected inner command may begin at a later argv position.
pub fn command_stages_match_wrapped_prefix(
    stages: &[CommandStageV1],
    prefix: &[String],
) -> PrefixMatch {
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
        if words
            .windows(prefix.len())
            .any(|candidate| candidate_matches_prefix(candidate, prefix))
        {
            return PrefixMatch::Matched;
        }
    }
    PrefixMatch::NotMatched
}

fn command_stages_match_prefix_impl(stages: &[CommandStageV1], prefix: &[String]) -> PrefixMatch {
    if std::env::var_os("ASP_TRACE_COMMAND_MATCH").is_some() {
        eprintln!(
            "[command-match-trace] prefix={prefix:?} stages={:?}",
            stages.iter().map(CommandStageV1::words).collect::<Vec<_>>()
        );
    }
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

        for candidate in words.windows(prefix.len()) {
            if inspected_candidates == MAX_COMMAND_CANDIDATES {
                return PrefixMatch::BudgetExceeded;
            }
            inspected_candidates += 1;
            if candidate_matches_prefix(candidate, prefix) {
                return PrefixMatch::Matched;
            }
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
    parse_bash_command_candidates_with_nested_scripts(command, 0)
}

fn parse_bash_command_candidates_with_nested_scripts(
    command: &str,
    depth: usize,
) -> Result<Vec<CommandStageV1>, String> {
    let mut candidates = if let Some(words) = simple_command_words(command) {
        vec![CommandStageV1 { words }]
    } else {
        parse_bash_command_candidates_impl(command)?
    };
    if depth >= 4 {
        return Ok(candidates);
    }
    // Wrapper discovery is structural and deliberately independent of the
    // wrapper executable's spelling. Tree-sitter-normalized quoted arguments
    // retain their internal whitespace, so they can be parsed into bounded
    // executable stages without enumerating wrapper names or paths. Policy
    // matching only observes the resulting argv stages.
    let scripts = candidates
        .iter()
        .flat_map(|candidate| nested_stage_arguments(candidate.words()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if scripts.is_empty() {
        return Ok(candidates);
    }
    for script in scripts {
        let Ok(nested_candidates) =
            parse_bash_command_candidates_with_nested_scripts(script.as_str(), depth + 1)
        else {
            continue;
        };
        for nested in nested_candidates {
            if candidates.len() >= MAX_COMMAND_CANDIDATES
                || candidates.iter().any(|candidate| candidate == &nested)
            {
                continue;
            }
            candidates.push(nested);
        }
    }
    Ok(candidates)
}

fn simple_command_words(command: &str) -> Option<Vec<String>> {
    if command.is_empty()
        || command.chars().any(|character| {
            matches!(
                character,
                '\\' | ';' | '&' | '|' | '(' | ')' | '$' | '<' | '>' | '\n' | '\r'
            )
        })
    {
        return None;
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    for character in command.chars() {
        match quote {
            Some(delimiter) if character == delimiter => quote = None,
            Some(_) => word.push(character),
            None if matches!(character, '\'' | '"') => quote = Some(character),
            None if character.is_ascii_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            None => word.push(character),
        }
    }
    if quote.is_some() {
        return None;
    }
    if !word.is_empty() {
        words.push(word);
    }
    (!words.is_empty() && words.len() <= MAX_STAGE_TOKENS).then_some(words)
}

fn nested_stage_arguments(words: &[String]) -> Vec<&str> {
    words
        .windows(2)
        .filter_map(|pair| {
            let option = &pair[0];
            let value = &pair[1];
            (option.starts_with('-') && option != "--" && value.chars().any(char::is_whitespace))
                .then_some(value.as_str())
        })
        .collect()
}

fn parse_bash_command_candidates_impl(command: &str) -> Result<Vec<CommandStageV1>, String> {
    let tokens = bash_parser::bash_ast_tokens(command)
        .ok_or_else(|| "bash-tree-sitter-parse-failed".to_string())?;
    let mut candidates = Vec::new();
    let mut executable_candidates = 0usize;
    for words in bash_parser::split_command_stages(tokens) {
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
        if !is_separator_stage {
            if executable_candidates > MAX_COMMAND_CANDIDATES {
                break;
            }
            executable_candidates += 1;
        }
        candidates.push(CommandStageV1 { words });
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

/// Parse and match a Bash command with generic wrapper-prefix discovery enabled.
pub fn match_bash_wrapped_command_prefix(command: &str, prefix: &[&str]) -> BashCommandMatchV1 {
    match parse_bash_command_candidates(command) {
        Ok(stages) => {
            let prefix = prefix
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();
            BashCommandMatchV1::Parsed(command_stages_match_wrapped_prefix(&stages, &prefix))
        }
        Err(_) => BashCommandMatchV1::InvalidSyntax {
            reason: "bash-tree-sitter-parse-failed",
        },
    }
}
