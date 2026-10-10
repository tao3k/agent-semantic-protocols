// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_execution_closure::{
    LanguageSchemaClosureEntry, NamedRuntimeDigestClosureEntry, RuntimeArtifactExecutionClosure,
    RuntimeArtifactExecutionClosureMember, RuntimeArtifactExecutionClosureMemberKind,
};
use agent_semantic_artifacts::runtime_artifact_slots::stage_runtime_artifact_bound_bundle_manifest;

use super::ASP_PYTHON_GRAPHS_BUNDLE_MEMBER;
use super::VerifiedAspPythonGraphsArtifact;

fn publish_bundle_fixture(state_home: &Path, members: &[(&str, &[u8])]) -> PathBuf {
    let candidate = state_home.join("runtime/artifacts/bundles/asp/test-bundle");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let mut digests = BTreeMap::new();
    for (name, bytes) in members {
        let path = candidate.join(name);
        std::fs::write(&path, bytes).expect("member bytes");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("member executable permission");
        digests.insert((*name).to_owned(), Blake3ContentDigest::from_bytes(bytes));
    }
    let closure = RuntimeArtifactExecutionClosure {
        provider_registration: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
            Vec::new(),
        ),
        provider_artifact_set: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
            Vec::new(),
        ),
        evaluator_policy: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
            vec![NamedRuntimeDigestClosureEntry {
                id: "graphs-policy".to_owned(),
                digest: Blake3ContentDigest::from_bytes(b"graphs-policy"),
            }],
        ),
        evaluator_abi: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
            vec![NamedRuntimeDigestClosureEntry {
                id: "graphs-abi".to_owned(),
                digest: Blake3ContentDigest::from_bytes(b"graphs-abi"),
            }],
        ),
        schema_bundle: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
            vec![LanguageSchemaClosureEntry {
                language_id: "python".to_owned(),
                schema_digest: Blake3ContentDigest::from_bytes(b"python-schemas"),
            }],
        ),
    };
    for (name, bytes) in closure.materialized_members().expect("closure members") {
        std::fs::write(candidate.join(name), &bytes).expect("write closure member");
        digests.insert(name.to_owned(), Blake3ContentDigest::from_bytes(&bytes));
    }
    stage_runtime_artifact_bound_bundle_manifest(
        &candidate,
        &digests,
        &closure.binding().expect("closure binding"),
    )
    .expect("bound bundle manifest");
    let active = state_home.join("runtime/artifacts/active");
    std::fs::create_dir_all(active.parent().expect("active parent")).expect("resident directory");
    symlink(&candidate, &active).expect("active selector");
    candidate
}

#[tokio::test]
async fn exact_active_bundle_member_is_the_only_graph_worker_authority() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let state_home = temporary.path();
    let candidate = publish_bundle_fixture(
        state_home,
        &[
            ("asp", b"asp"),
            (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
        ],
    );

    let artifact = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(state_home)
        .await
        .expect("verified graph member");

    assert_eq!(
        artifact.executable,
        candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER)
    );
    assert_eq!(
        artifact.content_digest,
        Blake3ContentDigest::from_bytes(b"graphs")
    );
}

#[tokio::test]
async fn missing_graph_member_is_typed_capability_unavailable() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    publish_bundle_fixture(temporary.path(), &[("asp", b"asp")]);

    let error = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
        .await
        .expect_err("missing optional member must fail closed");

    assert!(error.contains("reasonKind=asp-python-graphs-bundle-member-not-installed"));
}

#[tokio::test]
async fn graph_member_digest_drift_fails_before_process_construction() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let candidate = publish_bundle_fixture(
        temporary.path(),
        &[
            ("asp", b"asp"),
            (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
        ],
    );
    std::fs::write(candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER), b"drift")
        .expect("corrupt graph member");

    let error = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
        .await
        .expect_err("digest drift must fail closed");

    assert!(error.contains("Runtime artifact bundle member digest mismatch"));
}
