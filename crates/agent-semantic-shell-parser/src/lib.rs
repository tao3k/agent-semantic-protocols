//! Shared command-prefix matching for hook rules and execution lanes.
//!
//! Wrapper matching is deliberately lexical: it never probes the filesystem,
//! resolves `PATH`, or starts another process. Every shell stage is scanned
//! for a bounded prefix window so bare commands, absolute executables, and
//! wrapper-prefixed commands have identical routing semantics.

#![deny(dead_code)]

/// Parser-owned Bash AST tokenization and shell-stage normalization.
mod bash_parser;
mod behavior_facts;
mod shell_stage_match;
pub use bash_parser::apply_patch_header_paths;
mod source_paths;
mod structured_projection;
pub use behavior_facts::{
    ShellAccessKind, ShellBehaviorEvidence, ShellBehaviorFact, command_stage_behavior_facts,
};
pub use shell_stage_match::{
    BashCommandMatch, CommandStage, MAX_COMMAND_CANDIDATES, MAX_STAGE_TOKENS, PrefixMatch,
    candidate_matches_prefix, command_stages_match_prefix,
    command_stages_match_process_environment_assignment, command_stages_match_wrapped_prefix,
    command_tokens_match_argv_pattern, match_bash_command_prefix,
    match_bash_wrapped_command_prefix, parse_bash_command_candidates, render_bash_command_stage,
};

pub use source_paths::{
    command_source_paths, command_stage_source_paths, embedded_literal_candidates,
    path_like_token_matches,
};

pub mod structured;

pub mod bash;
