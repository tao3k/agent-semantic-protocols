// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Document traversal configuration used by the in-process Org/Markdown provider.

#[derive(Debug, Clone, Default)]
pub(super) struct AspConfig {
    pub(super) search: SearchConfig,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SearchConfig {
    pub(super) ignore_dirs: Vec<String>,
    pub(super) include_hidden_dirs: Vec<String>,
}
