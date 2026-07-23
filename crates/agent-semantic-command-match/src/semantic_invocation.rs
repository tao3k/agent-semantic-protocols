//! Semantic command-invocation normalization and wrapper facts.

/// Normalized command-invocation shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandInvocationShapeV1 {
    Command,
    WrappedCommand,
}

/// Whether a configured command wrapper matched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandWrapperMatchV1 {
    Matched,
    Unmatched,
}

/// Whether normalized command flags are present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandFlagPresenceV1 {
    Present,
    Absent,
}

/// Configured executable that may wrap another command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandWrapperSpecV1 {
    pub executable: String,
}

/// Parser-owned semantic projection of one command invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCommandInvocationV1 {
    pub shape: CommandInvocationShapeV1,
    pub wrapper_match: CommandWrapperMatchV1,
    pub wrapper_chain: Vec<String>,
    pub argv: Vec<String>,
    pub executable: Option<String>,
    pub flags: Vec<String>,
    pub flag_presence: CommandFlagPresenceV1,
    pub operands: Vec<String>,
}

/// Normalize a Bash command into semantic invocation candidates.
pub fn normalize_bash_command_invocations(
    command: &str,
    wrappers: &[CommandWrapperSpecV1],
) -> Result<Vec<SemanticCommandInvocationV1>, String> {
    crate::parse_bash_command_candidates(command).map(|stages| {
        stages
            .into_iter()
            .filter_map(|stage| normalize_stage(stage, wrappers))
            .collect()
    })
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn normalize_stage(
    stage: crate::CommandStageV1,
    wrappers: &[CommandWrapperSpecV1],
) -> Option<SemanticCommandInvocationV1> {
    let crate::CommandStageV1 { words } = stage;
    if words.is_empty() {
        return None;
    }

    let mut cursor = 0;
    let mut wrapper_chain = Vec::new();
    let mut flags = Vec::new();

    while cursor < words.len() && is_registered_wrapper(&words[cursor], wrappers) {
        wrapper_chain.push(words[cursor].clone());
        cursor += 1;
        while cursor < words.len() && is_flag(&words[cursor]) {
            flags.push(words[cursor].clone());
            cursor += 1;
        }
        while cursor < words.len() && is_environment_assignment(&words[cursor]) {
            cursor += 1;
        }
    }

    while cursor < words.len() && is_environment_assignment(&words[cursor]) {
        cursor += 1;
    }

    let argv = words[cursor..].to_vec();
    let executable = words.get(cursor).cloned();
    if executable.is_some() {
        cursor += 1;
    }

    let mut operands = Vec::new();
    for word in &words[cursor..] {
        if is_flag(word) {
            flags.push(word.clone());
        } else {
            operands.push(word.clone());
        }
    }

    let wrapper_match = if wrapper_chain.is_empty() {
        CommandWrapperMatchV1::Unmatched
    } else {
        CommandWrapperMatchV1::Matched
    };
    let shape = if wrapper_chain.is_empty() {
        CommandInvocationShapeV1::Command
    } else {
        CommandInvocationShapeV1::WrappedCommand
    };
    let flag_presence = if flags.is_empty() {
        CommandFlagPresenceV1::Absent
    } else {
        CommandFlagPresenceV1::Present
    };

    Some(SemanticCommandInvocationV1 {
        shape,
        wrapper_match,
        wrapper_chain,
        argv,
        executable,
        flags,
        flag_presence,
        operands,
    })
}

/// Match semantic invocation candidates against a configured argv prefix.
pub fn semantic_invocations_match_prefix(
    invocations: &[SemanticCommandInvocationV1],
    prefix: &[String],
) -> crate::PrefixMatch {
    semantic_invocations_match_prefix_impl(invocations, prefix)
}

fn semantic_invocations_match_prefix_impl(
    invocations: &[SemanticCommandInvocationV1],
    prefix: &[String],
) -> crate::PrefixMatch {
    if prefix.is_empty() {
        return crate::PrefixMatch::Matched;
    }
    let mut inspected_candidates = 0usize;
    for invocation in invocations {
        if invocation.argv.len() > crate::MAX_STAGE_TOKENS {
            return crate::PrefixMatch::BudgetExceeded;
        }
        if invocation.argv.len() < prefix.len() {
            continue;
        }
        if inspected_candidates == crate::MAX_COMMAND_CANDIDATES {
            return crate::PrefixMatch::BudgetExceeded;
        }
        inspected_candidates += 1;
        if crate::candidate_matches_prefix(&invocation.argv, prefix) {
            return crate::PrefixMatch::Matched;
        }
    }
    crate::PrefixMatch::NotMatched
}

fn is_registered_wrapper(token: &str, wrappers: &[CommandWrapperSpecV1]) -> bool {
    let basename = crate::command_token_basename(token);
    wrappers
        .iter()
        .any(|wrapper| crate::command_token_basename(&wrapper.executable) == basename)
}

fn is_flag(token: &str) -> bool {
    token != "-" && token.starts_with('-')
}

#[cfg(test)]
#[path = "../tests/unit/semantic_invocation.rs"]
mod tests;
