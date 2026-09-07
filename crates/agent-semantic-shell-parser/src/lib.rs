// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
pub use behavior_facts::ShellAccessKind;
pub use behavior_facts::ShellBehaviorEvidence;
pub use behavior_facts::ShellBehaviorFact;
pub use behavior_facts::command_stage_behavior_facts;
pub use shell_stage_match::BashCommandMatch;
pub use shell_stage_match::CommandStage;
pub use shell_stage_match::MAX_COMMAND_CANDIDATES;
pub use shell_stage_match::MAX_STAGE_TOKENS;
pub use shell_stage_match::PrefixMatch;
pub use shell_stage_match::candidate_matches_prefix;
pub use shell_stage_match::command_stages_match_prefix;
pub use shell_stage_match::command_stages_match_process_environment_assignment;
pub use shell_stage_match::command_stages_match_wrapped_prefix;
pub use shell_stage_match::command_tokens_match_argv_pattern;
pub use shell_stage_match::match_bash_command_prefix;
pub use shell_stage_match::match_bash_wrapped_command_prefix;
pub use shell_stage_match::parse_bash_command_candidates;
pub use shell_stage_match::render_bash_command_stage;

pub use source_paths::command_source_paths;
pub use source_paths::command_stage_source_paths;
pub use source_paths::embedded_literal_candidates;
pub use source_paths::path_like_token_matches;

pub mod structured;

pub mod bash;
