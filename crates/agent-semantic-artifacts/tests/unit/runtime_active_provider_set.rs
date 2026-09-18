fn write_source(directory: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("Runtime bundle source");
    path
}

#[tokio::test]
async fn provider_set_is_derived_from_active_bundle_without_a_side_document() {
    let state_home = tempfile::tempdir().expect("state home");
    let sources = state_home.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let asp = sources.join("asp");
    let hook = sources.join("asp-hook");
    let rust = sources.join("asp-rust");
    std::fs::write(&asp, b"asp").expect("asp");
    std::fs::write(&hook, b"hook").expect("hook");
    std::fs::write(&rust, b"rust").expect("rust");
    let source_members = std::collections::BTreeMap::from([
        (
            "asp".to_owned(),
            crate::runtime_artifact_store::runtime_artifact_content_digest(&asp)
                .expect("asp digest"),
        ),
        (
            "asp-hook".to_owned(),
            crate::runtime_artifact_store::runtime_artifact_content_digest(&hook)
                .expect("hook digest"),
        ),
        (
            "asp-rust".to_owned(),
            crate::runtime_artifact_store::runtime_artifact_content_digest(&rust)
                .expect("Rust digest"),
        ),
    ]);
    let closure = crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_runtime_bundle_members(
        &source_members,
        vec![crate::runtime_artifact_execution_closure::NamedRuntimeDigestClosureEntry {
            id: "query-admission".into(),
            digest: crate::blake3_content_digest::Blake3ContentDigest::from_bytes(b"policy"),
        }],
        vec![crate::runtime_artifact_execution_closure::NamedRuntimeDigestClosureEntry {
            id: "query-playbook-v1".into(),
            digest: crate::blake3_content_digest::Blake3ContentDigest::from_bytes(b"abi"),
        }],
        vec![crate::runtime_artifact_execution_closure::LanguageSchemaClosureEntry {
            language_id: "rust".into(),
            schema_digest: crate::blake3_content_digest::Blake3ContentDigest::from_bytes(b"schemas"),
        }],
    )
    .expect("execution closure");
    let closure_sources = closure
        .materialized_members()
        .expect("closure members")
        .into_iter()
        .map(|(name, bytes)| (name, write_source(&sources, name, &bytes)))
        .collect::<Vec<_>>();
    let mut members = vec![
        crate::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: &hook,
        },
        crate::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-rust",
            source: &rust,
        },
    ];
    members.extend(closure_sources.iter().map(|(name, source)| {
        crate::runtime_artifact_publication::RuntimeArtifactBundleMemberSource { name, source }
    }));
    let execution_binding = closure.binding().expect("execution binding");
    let published =
        crate::runtime_artifact_publication::publish_runtime_artifact_bound_bundle_members(
            state_home.path(),
            &asp,
            &state_home.path().join("runtime/bin/asp"),
            "dev",
            &members,
            &execution_binding,
        )
        .await
        .expect("publish bundle");

    let providers = super::load_active_runtime_bound_provider_set(state_home.path())
        .expect("active provider set");
    assert_eq!(
        providers.runtime_bundle_digest,
        published.bundle_digest.to_string()
    );
    assert_eq!(providers.providers.len(), 1);
    assert_eq!(providers.providers[0].provider_id, "asp-rust");
    assert_eq!(providers.execution_binding, execution_binding);
    assert!(
        providers.providers[0].materialized_path.starts_with(
            crate::RuntimeArtifactStateLayout::new(state_home.path())
                .generation_store()
                .canonicalize()
                .expect("canonical generation store")
        )
    );
    assert!(
        !state_home
            .path()
            .join("runtime/installed-provider-artifacts.json")
            .exists()
    );
    assert!(
        !state_home
            .path()
            .join("runtime/installed-provider-binding.v1.json")
            .exists()
    );
}

#[tokio::test]
async fn unbound_active_bundle_is_not_a_serving_provider_authority() {
    let state_home = tempfile::tempdir().expect("state home");
    let sources = state_home.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let asp = write_source(&sources, "asp", b"asp");
    crate::runtime_artifact_publication::publish_runtime_artifact_bundle_members(
        state_home.path(),
        &asp,
        &state_home.path().join("runtime/bin/asp"),
        "dev",
        &[],
    )
    .await
    .expect("publish historical unbound fixture");

    let error = super::load_active_runtime_bound_provider_set(state_home.path())
        .expect_err("serving admission must reject a members-only Runtime bundle");
    assert!(error.contains("reasonKind=runtime-bundle-binding-missing"));
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
