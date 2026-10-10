// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned execution boundary for resident Search acquisition engines.

#[path = "runtime_resident_grep.rs"]
mod resident_grep;

pub(crate) use resident_grep::RuntimeGrepMatch;
pub(crate) use resident_grep::execute_runtime_resident_grep_blocks;
