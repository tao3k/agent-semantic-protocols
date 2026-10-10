// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public Bash parsing and command-stage facade.

pub use crate::bash_parser::apply_patch_header_paths;
pub use crate::bash_parser::command_name;
pub use crate::bash_parser::is_separator;
pub use crate::bash_parser::semantic_shell_stages;
pub use crate::bash_parser::shell_tokens;
pub use crate::bash_parser::split_command_stages;
pub use crate::parse_bash_command_candidates;
