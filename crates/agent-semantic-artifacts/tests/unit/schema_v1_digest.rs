// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::Blake3ContentDigest;

#[test]
fn canonicalizes_schema_v1_digest_inputs() {
    let canonical = format!("blake3-256:{}", "a".repeat(64));
    for input in [
        serde_json::json!(canonical),
        serde_json::json!("a".repeat(64)),
        serde_json::json!({
            "value": "a".repeat(64),
            "algorithm": "blake3-256"
        }),
    ] {
        let digest: Blake3ContentDigest = serde_json::from_value(input).unwrap();
        assert_eq!(digest.to_string(), format!("blake3-256:{}", "a".repeat(64)));
    }
}

#[test]
fn malformed_schema_v1_digest_is_rejected() {
    let error = serde_json::from_value::<Blake3ContentDigest>(serde_json::json!({
        "value": "NOT-A-DIGEST",
        "algorithm": "blake3-256"
    }))
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reasonKind=artifact-identity-incomplete")
    );
}
