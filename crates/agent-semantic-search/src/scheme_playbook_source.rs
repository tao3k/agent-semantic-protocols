// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Search-owned configuration and Tree-sitter structural admission.

pub use agent_semantic_tree_sitter::{SchemeSourceAdmission, SchemeSourceAdmissionError};

pub const DEFAULT_SEARCH_PLAYBOOK_SOURCE: &str = include_str!("../scheme/search-playbook.ss");

pub fn admit_search_playbook_source(
    source: &str,
) -> Result<SchemeSourceAdmission, SchemeSourceAdmissionError> {
    agent_semantic_tree_sitter::admit_scheme_source(source)
}

pub fn admit_default_search_playbook_source()
-> Result<SchemeSourceAdmission, SchemeSourceAdmissionError> {
    admit_search_playbook_source(DEFAULT_SEARCH_PLAYBOOK_SOURCE)
}
