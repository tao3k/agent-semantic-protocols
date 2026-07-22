//! Shared command-prefix matching for hook rules and execution lanes.
//!
//! Wrapper matching is deliberately lexical: it never probes the filesystem,
//! resolves `PATH`, or starts another process. Every shell stage is scanned
//! for a bounded prefix window so bare commands, absolute executables, and
//! wrapper-prefixed commands have identical routing semantics.

#![deny(dead_code)]

pub const MAX_STAGE_TOKENS: usize = 256;
pub const MAX_COMMAND_CANDIDATES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
    Matched,
    NotMatched,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStageV1 {
    words: Vec<String>,
}

impl CommandStageV1 {
    pub fn new(words: Vec<String>) -> Self {
        Self { words }
    }

    pub fn words(&self) -> &[String] {
        &self.words
    }

    /// Executable word for this parsed shell stage.
    pub fn executable(&self) -> Option<&str> {
        self.words.first().map(String::as_str)
    }
}

impl PrefixMatch {
    /// Budget exhaustion is protected routing, never an escape hatch.
    pub const fn routes_protected(self) -> bool {
        !matches!(self, Self::NotMatched)
    }
}

pub fn command_stages_match_prefix(stages: &[CommandStageV1], prefix: &[String]) -> PrefixMatch {
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

fn command_token_basename(token: &str) -> &str {
    token.rsplit(['/', '\\']).next().unwrap_or(token)
}

mod bash_parser;
mod semantic_invocation;
pub use semantic_invocation::{
    CommandFlagPresenceV1, CommandInvocationShapeV1, CommandWrapperMatchV1, CommandWrapperSpecV1,
    SemanticCommandInvocationV1, normalize_bash_command_invocations,
    semantic_invocations_match_prefix,
};
mod source_paths;
mod structured_projection;

pub use source_paths::{command_source_paths, path_like_token_matches};

pub mod structured {
    pub use crate::structured_projection::{
        BoundedPathCommandSpecV1, BoundedPathSegmentV1, StructuredFilterClassificationV1,
        classify_bounded_path_filter, classify_single_bounded_path_command,
    };
}

pub mod bash {
    pub use crate::bash_parser::*;
    pub use crate::parse_bash_command_candidates;
}

#[derive(Debug, PartialEq)]
pub enum BashCommandMatchV1 {
    Parsed(PrefixMatch),
    InvalidSyntax { reason: &'static str },
}

pub fn parse_bash_command_candidates(command: &str) -> Result<Vec<CommandStageV1>, String> {
    let tokens = bash_parser::bash_ast_tokens(command)
        .ok_or_else(|| "bash-tree-sitter-parse-failed".to_string())?;
    let raw_stages = bash_parser::split_command_stages(tokens);
    let mut pending = std::collections::VecDeque::from(raw_stages);
    let mut candidates = Vec::new();
    while let Some(words) = pending.pop_front() {
        if words.is_empty() {
            continue;
        }
        if candidates
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
