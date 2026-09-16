fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn binding(runtime_bundle_digest: String) -> super::RuntimeProviderExecutionBinding {
    super::RuntimeProviderExecutionBinding::build(
        "repo-project".to_owned(),
        "workspace-project".to_owned(),
        runtime_bundle_digest,
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
    .expect("binding")
}

#[test]
fn execution_identity_changes_with_the_active_runtime_bundle() {
    assert_ne!(
        binding(digest('a')).generation,
        binding(digest('f')).generation
    );
}

#[test]
fn activation_sequence_cannot_substitute_for_runtime_content_identity() {
    let error = super::RuntimeProviderExecutionBinding::build(
        "repo-project".to_owned(),
        "workspace-project".to_owned(),
        "84".to_owned(),
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
    .expect_err("publication sequence is not a content digest");
    assert!(error.contains("runtimeBundleDigest"), "{error}");
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
