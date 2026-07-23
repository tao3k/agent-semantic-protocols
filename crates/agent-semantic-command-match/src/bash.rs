//! Public Bash parsing and command-stage facade.

pub use crate::bash_parser::{
    apply_patch_header_paths, command_name, is_separator, semantic_shell_stages, shell_tokens,
    split_command_stages, unwrap_command_stage,
};
pub use crate::parse_bash_command_candidates;
