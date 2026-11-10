// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{DEFAULT_SEARCH_PLAYBOOK_SOURCE, admit_default_search_playbook_source};

#[test]
fn default_source_is_a_small_macro_declaration() {
    let admission = admit_default_search_playbook_source().expect("Tree-sitter admission");
    assert_eq!(
        admission.top_level_form_heads,
        vec!["import", "export", "defsearch-playbook"]
    );
    assert!(DEFAULT_SEARCH_PLAYBOOK_SOURCE.contains("(intersect"));
    assert!(!DEFAULT_SEARCH_PLAYBOOK_SOURCE.contains("external-flow"));
}
