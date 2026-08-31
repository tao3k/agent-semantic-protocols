//! Bounded shell-stage and prefix matching contracts.

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
pub struct CommandStage {
    pub(crate) words: Vec<String>,
}

impl CommandStage {
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

    /// Whether this stage is a shell control separator rather than an argv.
    pub fn is_separator(&self) -> bool {
        self.words.len() == 1 && bash_parser::is_separator(&self.words[0])
    }
}

/// Render one normalized stage back to a shell string without changing argv
/// boundaries. This is used only to hand a parser-owned stage to downstream
/// declarative matchers; it never executes the rendered command.
pub fn render_bash_command_stage(stage: &CommandStage) -> String {
    stage
        .words()
        .iter()
        .map(|word| {
            if !word.is_empty()
                && word.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'_' | b'-' | b'.' | b'/' | b':' | b'=' | b'@' | b'%')
                })
            {
                word.clone()
            } else {
                format!("'{}'", word.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Match normalized command stages against a configured argv prefix.
pub fn command_stages_match_prefix(stages: &[CommandStage], prefix: &[String]) -> PrefixMatch {
    command_stages_match_prefix_impl(stages, prefix)
}

/// Match a configured command prefix at any bounded argv position in a parsed stage.
///
/// This is the generic wrapper mode: the outer executable remains parser-visible,
/// while a protected inner command may begin at a later argv position.
pub fn command_stages_match_wrapped_prefix(
    stages: &[CommandStage],
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

/// Match an exact process-environment assignment carried by one of the three
/// bounded shell forms that preserve it for the target process:
///
/// - `NAME=VALUE command ...`
/// - `/usr/bin/env NAME=VALUE command ...`
/// - `export NAME=VALUE; command ...`
///
/// This is a parser fact, not a substring check. An assignment in an unrelated
/// stage, a nested shell string, or a non-`exec` continuation does not match.
pub fn command_stages_match_process_environment_assignment<S: AsRef<str>>(
    stages: &[CommandStage],
    expected: &[S],
) -> bool {
    let command_stages = stages
        .iter()
        .filter(|stage| !stage.is_separator())
        .collect::<Vec<_>>();
    let Some(first_stage) = command_stages.first() else {
        return false;
    };
    if first_stage
        .words()
        .iter()
        .take_while(|word| is_environment_assignment(word))
        .any(|assignment| {
            expected
                .iter()
                .any(|expected| assignment == expected.as_ref())
        })
    {
        return true;
    }

    if first_stage
        .executable()
        .is_some_and(|word| command_token_basename(word) == "env")
        && first_stage
            .words()
            .iter()
            .skip(1)
            .take_while(|word| is_environment_assignment(word))
            .any(|assignment| {
                expected
                    .iter()
                    .any(|expected| assignment == expected.as_ref())
            })
    {
        return true;
    }

    if command_stages.len() < 2
        || first_stage
            .executable()
            .is_none_or(|word| command_token_basename(word) != "export")
    {
        return false;
    }
    first_stage
        .words()
        .iter()
        .skip(1)
        .all(|word| is_environment_assignment(word))
        && first_stage.words().iter().skip(1).any(|assignment| {
            expected
                .iter()
                .any(|expected| assignment == expected.as_ref())
        })
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn command_stages_match_prefix_impl(stages: &[CommandStage], prefix: &[String]) -> PrefixMatch {
    if std::env::var_os("ASP_TRACE_SHELL_PARSER").is_some() {
        eprintln!(
            "[shell-parser-trace] prefix={prefix:?} stages={:?}",
            stages.iter().map(CommandStage::words).collect::<Vec<_>>()
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

/// Match a declarative argv glob pattern against a normalized command stage.
///
/// The executable is compared by basename so absolute paths and wrapper
/// prefixes remain transparent. `*` and `**` are argv-sequence wildcards that
/// consume zero or more tokens; every other pattern token is compiled by
/// globset and consumes exactly one token. Once the declared prefix matches,
/// additional argv are intentionally ignored.
pub fn command_tokens_match_argv_pattern(
    tokens: &[String],
    pattern: &[String],
    wrapped_command: bool,
    boundary_token: &str,
) -> bool {
    if tokens.is_empty() || pattern.is_empty() {
        return false;
    }
    let boundary = tokens
        .iter()
        .position(|token| token == boundary_token)
        .unwrap_or(tokens.len());
    let candidate_end = if boundary < tokens.len() {
        boundary + 1
    } else {
        tokens.len()
    };
    let starts = if wrapped_command { 0..boundary } else { 0..1 };
    starts.into_iter().any(|start| {
        let Some(actual) = tokens.get(start..candidate_end) else {
            return false;
        };
        argv_pattern_matches(actual, pattern)
    })
}

fn argv_pattern_matches(actual: &[String], pattern: &[String]) -> bool {
    let mut actual_index = 0usize;
    let mut pattern_index = 0usize;
    let mut sequence_wildcard = None;
    let mut wildcard_consumed = 0usize;

    loop {
        if pattern_index == pattern.len() {
            return true;
        }
        if matches!(pattern[pattern_index].as_str(), "*" | "**") {
            sequence_wildcard = Some(pattern_index);
            wildcard_consumed = actual_index;
            pattern_index += 1;
            continue;
        }
        if let Some(candidate) = actual.get(actual_index) {
            let candidate = if actual_index == 0 {
                command_token_basename(candidate)
            } else {
                candidate.as_str()
            };
            if argv_token_glob_matches(&pattern[pattern_index], candidate) {
                actual_index += 1;
                pattern_index += 1;
                continue;
            }
        }
        let Some(wildcard_index) = sequence_wildcard else {
            return false;
        };
        if wildcard_consumed == actual.len() {
            return false;
        }
        wildcard_consumed += 1;
        actual_index = wildcard_consumed;
        pattern_index = wildcard_index + 1;
    }
}

fn argv_token_glob_matches(pattern: &str, actual: &str) -> bool {
    if !pattern
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']' | b'{' | b'}'))
    {
        return pattern == actual;
    }
    globset::Glob::new(pattern)
        .map(|glob| glob.compile_matcher().is_match(actual))
        .unwrap_or(false)
}

/// Parse a Bash command into bounded normalized command candidates.
pub fn parse_bash_command_candidates(command: &str) -> Result<Vec<CommandStage>, String> {
    parse_bash_command_candidates_with_shell_operands(command, 0)
}

fn parse_bash_command_candidates_with_shell_operands(
    command: &str,
    depth: usize,
) -> Result<Vec<CommandStage>, String> {
    let mut candidates = if let Some(words) = simple_command_words(command) {
        vec![CommandStage { words }]
    } else {
        parse_bash_command_candidates_impl(command)?
    };
    if depth >= 4 {
        return Ok(candidates);
    }
    let scripts = candidates
        .iter()
        .flat_map(|candidate| shell_execution_operands(candidate.words()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for script in scripts {
        for nested in parse_bash_command_candidates_with_shell_operands(&script, depth + 1)? {
            if candidates.len() >= MAX_COMMAND_CANDIDATES {
                return Ok(candidates);
            }
            if !candidates.contains(&nested) {
                candidates.push(nested);
            }
        }
    }
    Ok(candidates)
}

fn shell_execution_operands(words: &[String]) -> Vec<&str> {
    let mut operands = Vec::new();
    let mut executable_index = 0usize;
    let Some(mut executable) = words.first().map(|word| command_token_basename(word)) else {
        return operands;
    };
    if executable == "env" {
        let Some(index) = words.iter().enumerate().skip(1).find_map(|(index, word)| {
            (!word.starts_with('-') && !word.contains('=')).then_some(index)
        }) else {
            return operands;
        };
        executable_index = index;
        executable = command_token_basename(&words[executable_index]);
    }
    if matches!(executable, "bash" | "sh" | "zsh")
        && let Some(script) = words[executable_index + 1..].windows(2).find_map(|pair| {
            let option = pair[0].as_str();
            let is_command_option = option == "--command"
                || (option.starts_with('-') && option.trim_start_matches('-').contains('c'));
            is_command_option.then_some(pair[1].as_str())
        })
    {
        operands.push(script);
    }
    if matches!(
        executable,
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) && let Some(script) = words[executable_index + 1..].windows(2).find_map(|pair| {
        matches!(
            pair[0].to_ascii_lowercase().as_str(),
            "-c" | "-command" | "-encodedcommand"
        )
        .then_some(pair[1].as_str())
    }) {
        operands.push(script);
    }

    // Generic command runners conventionally carry their shell program in the
    // argv immediately following the `run` subcommand.  Recognize that shape,
    // rather than wrapper executable names, so project-local launchers and
    // future wrappers receive the same bounded recursive parse as `sh -c`.
    // A program must contain whitespace or shell control syntax; a bare task
    // label is not reinterpreted as another executable.
    operands.extend(words.windows(2).filter_map(|pair| {
        (pair[0] == "run"
            && pair[1].chars().any(|character| {
                character.is_ascii_whitespace() || ";|&<>\n\r".contains(character)
            }))
        .then_some(pair[1].as_str())
    }));
    operands.sort_unstable();
    operands.dedup();
    operands
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

fn parse_bash_command_candidates_impl(command: &str) -> Result<Vec<CommandStage>, String> {
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
                .any(|candidate: &CommandStage| candidate.words == words)
        {
            continue;
        }
        if !is_separator_stage {
            if executable_candidates > MAX_COMMAND_CANDIDATES {
                break;
            }
            executable_candidates += 1;
        }
        candidates.push(CommandStage { words });
    }
    (!candidates.is_empty())
        .then_some(candidates)
        .ok_or_else(|| "bash-tree-sitter-empty-command".to_string())
}

/// Public Bash prefix-match result including fail-closed parser errors.
#[derive(Debug, PartialEq)]
pub enum BashCommandMatch {
    Parsed(PrefixMatch),
    InvalidSyntax { reason: &'static str },
}

/// Parse and match a Bash command against a string-slice argv prefix.
pub fn match_bash_command_prefix(command: &str, prefix: &[&str]) -> BashCommandMatch {
    match parse_bash_command_candidates(command) {
        Ok(stages) => {
            let prefix = prefix
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();
            BashCommandMatch::Parsed(command_stages_match_prefix(&stages, &prefix))
        }
        Err(_) => BashCommandMatch::InvalidSyntax {
            reason: "bash-tree-sitter-parse-failed",
        },
    }
}

/// Parse and match a Bash command with generic wrapper-prefix discovery enabled.
pub fn match_bash_wrapped_command_prefix(command: &str, prefix: &[&str]) -> BashCommandMatch {
    match parse_bash_command_candidates(command) {
        Ok(stages) => {
            let prefix = prefix
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();
            BashCommandMatch::Parsed(command_stages_match_wrapped_prefix(&stages, &prefix))
        }
        Err(_) => BashCommandMatch::InvalidSyntax {
            reason: "bash-tree-sitter-parse-failed",
        },
    }
}
