// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::qualified_staged_development_provider;

#[test]
fn drifted_provider_reconciliation_requires_a_current_qualified_staged_receipt() {
    let temporary = tempfile::tempdir().expect("reconciliation fixture");
    let state_home = temporary.path().join("state");
    let developer_root = temporary.path().join("checkout");
    std::fs::create_dir_all(&developer_root).expect("developer root");
    let registration =
        crate::command::provider_install_registry::provider_install_registration("rust")
            .expect("Rust install registration");
    let provider_digest =
        crate::command::provider_install_registry::provider_install_registration_digest(
            &registration,
        )
        .expect("registration digest");
    let layout = agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&state_home);
    let artifact_dir = layout
        .provider_staging()
        .join("asp-rust/artifacts/blake3-merkle-v1/fixture");
    std::fs::create_dir_all(&artifact_dir).expect("artifact directory");
    let package = artifact_dir.join("root");
    let launcher = artifact_dir.join("launcher");
    std::fs::write(&package, b"provider").expect("provider artifact");
    std::fs::write(&launcher, b"#!/bin/sh\nexit 0\n").expect("provider launcher");
    let (artifact_digest, artifact_leaf_count) =
        super::super::workspace::artifact_snapshot(&package).expect("artifact snapshot");
    let launcher_digest = agent_semantic_content_identity::file_content_digest_v1(&launcher)
        .expect("launcher digest");
    let receipt_dir = layout.provider_staging().join("receipts");
    std::fs::create_dir_all(&receipt_dir).expect("receipt directory");
    let receipt_path = receipt_dir.join("rust.lock.toml");
    std::fs::write(
        &receipt_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nscope = \"state-home\"\nlanguage = \"rust\"\nprovider = \"asp-rust\"\nsourceKind = \"develop-workspace-tree\"\ncheckoutRoot = \"{}\"\npackagePath = \"{}\"\nproviderDigest = \"{}\"\nartifactDigest = \"{}\"\nartifactLeafCount = {}\nlauncherDigest = \"{}\"\n",
            developer_root.display(),
            package.display(),
            provider_digest,
            artifact_digest,
            artifact_leaf_count,
            launcher_digest,
        ),
    )
    .expect("provider receipt");

    assert_eq!(
        qualified_staged_development_provider(&state_home, &developer_root, "rust", "asp-rust",)
            .expect("qualified staged Provider"),
        launcher.canonicalize().unwrap()
    );
    let stale = std::fs::read_to_string(&receipt_path)
        .unwrap()
        .replace(&provider_digest, "sha256:stale");
    std::fs::write(&receipt_path, stale).unwrap();
    let error =
        qualified_staged_development_provider(&state_home, &developer_root, "rust", "asp-rust")
            .expect_err("stale registration receipt");
    assert!(error.contains("receipt-registration-drift"));
}
