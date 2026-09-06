// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::compact_root_source_access_message;

#[test]
fn structured_source_read_uses_configured_bounded_projector_message() {
    let decision = serde_json::json!({
        "reasonKind": "structured-source-read",
        "subject": { "paths": ["schemas/example.json"] },
        "routes": [],
        "fields": {
            "filterGrammar": "bounded-path-v1",
            "jsonBinary": "jq",
            "tomlBinary": "yq"
        }
    });

    let message = compact_root_source_access_message(&decision, "asp-explore")
        .expect("structured source read compact message");

    assert!(message.contains("`jq` structured reader"), "{message}");
    assert!(message.contains("`bounded-path-v1`"), "{message}");
    assert!(message.contains("`schemas/example.json`"), "{message}");
    assert!(!message.contains("resume resident"), "{message}");
}
