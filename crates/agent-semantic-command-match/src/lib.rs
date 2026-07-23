//! Shared command-prefix matching for hook rules and execution lanes.
//!
//! Wrapper matching is deliberately lexical: it never probes the filesystem,
//! resolves `PATH`, or starts another process. Every shell stage is scanned
//! for a bounded prefix window so bare commands, absolute executables, and
//! wrapper-prefixed commands have identical routing semantics.

#![deny(dead_code)]

/// Parser-owned Bash AST tokenization and shell-stage normalization.
mod bash_parser;
mod command_match;
pub use bash_parser::apply_patch_header_paths;
mod semantic_invocation;
pub use semantic_invocation::{
    CommandFlagPresenceV1, CommandInvocationShapeV1, CommandWrapperMatchV1, CommandWrapperSpecV1,
    SemanticCommandInvocationV1, normalize_bash_command_invocations,
    semantic_invocations_match_prefix,
};
mod source_paths;
mod structured_projection;
pub(crate) use command_match::command_token_basename;
pub use command_match::{
    BashCommandMatchV1, CommandStageV1, MAX_COMMAND_CANDIDATES, MAX_STAGE_TOKENS, PrefixMatch,
    candidate_matches_prefix, command_stages_match_prefix, match_bash_command_prefix,
    parse_bash_command_candidates,
};

pub use source_paths::{
    command_source_paths, embedded_literal_candidates, path_like_token_matches,
};

pub mod structured;

pub mod bash;
