// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::source_extension::source_extensions_support_file;
use std::path::Path;

#[test]
fn provider_schema_extensions_accept_dotless_and_dotted_names() {
    let extensions = vec!["rs".to_string(), ".py".to_string()];

    assert!(source_extensions_support_file(
        &extensions,
        Path::new("src/LIB.RS")
    ));
    assert!(source_extensions_support_file(
        &extensions,
        Path::new("src/main.py")
    ));
    assert!(!source_extensions_support_file(
        &extensions,
        Path::new("src/main.ts")
    ));
    assert!(!source_extensions_support_file(
        &extensions,
        Path::new("Makefile")
    ));
}
