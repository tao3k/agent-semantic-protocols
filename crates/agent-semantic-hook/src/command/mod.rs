// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shell command normalization and semantic-search routing helpers.

mod apply_patch;

mod shell;

pub(crate) use apply_patch::apply_patch_source_paths;
pub use shell::semantic_shell_tokens;
