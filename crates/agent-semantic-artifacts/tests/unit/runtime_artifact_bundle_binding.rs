// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use super::{
    RuntimeArtifactBundleBinding, runtime_artifact_bound_bundle_digest,
    runtime_artifact_bundle_digest, runtime_artifact_candidate_digest,
    stage_runtime_artifact_bound_bundle_manifest, verify_runtime_artifact_bound_bundle,
};
use crate::runtime_artifact_store::runtime_artifact_content_digest;

fn digest(label: &str) -> crate::blake3_content_digest::Blake3ContentDigest {
    crate::blake3_content_digest::Blake3ContentDigest::from_bytes(label.as_bytes())
}

fn write_valid_closure(
    candidate: &Path,
    members: &mut std::collections::BTreeMap<
        String,
        crate::blake3_content_digest::Blake3ContentDigest,
    >,
) -> RuntimeArtifactBundleBinding {
    use crate::runtime_artifact_execution_closure::{
        LanguageSchemaClosureEntry, NamedRuntimeDigestClosureEntry, ProviderArtifactClosureEntry,
        ProviderRegistrationClosureEntry, RuntimeArtifactExecutionClosure,
        RuntimeArtifactExecutionClosureMember, RuntimeArtifactExecutionClosureMemberKind,
    };

    let registration = agent_semantic_provider_protocol::builtin_provider_registrations()
        .expect("provider registrations")
        .into_iter()
        .find(|registration| registration.provider_id == "asp-rust")
        .expect("Rust registration");
    let artifact_member = "asp-rust".to_owned();
    let artifact_path = candidate.join(&artifact_member);
    std::fs::write(&artifact_path, b"rust provider").expect("provider member");
    let artifact_digest = runtime_artifact_content_digest(&artifact_path).expect("digest");
    members.insert(artifact_member.clone(), artifact_digest.clone());
    let closure = RuntimeArtifactExecutionClosure {
        provider_registration: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
            vec![ProviderRegistrationClosureEntry {
                provider_id: registration.provider_id,
                language_id: registration.language_id,
                registration_digest: crate::blake3_content_digest::Blake3ContentDigest::from_bytes(
                    &serde_json::to_vec(&registration.registration).expect("registration"),
                ),
                artifact_member: artifact_member.clone(),
            }],
        ),
        provider_artifact_set: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
            vec![ProviderArtifactClosureEntry {
                provider_id: "asp-rust".into(),
                artifact_member,
                artifact_digest,
            }],
        ),
        evaluator_policy: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
            vec![NamedRuntimeDigestClosureEntry {
                id: "query-admission".into(),
                digest: digest("policy"),
            }],
        ),
        evaluator_abi: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
            vec![NamedRuntimeDigestClosureEntry {
                id: "query-playbook-v1".into(),
                digest: digest("abi"),
            }],
        ),
        schema_bundle: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
            vec![LanguageSchemaClosureEntry {
                language_id: "rust".into(),
                schema_digest: digest("schemas"),
            }],
        ),
    };
    let binding = closure.binding().expect("closure binding");
    for (name, bytes) in closure.materialized_members().expect("closure members") {
        std::fs::write(candidate.join(name), &bytes).expect("closure member");
        members.insert(name.to_owned(), digest_member_bytes(&bytes));
    }
    binding
}

fn digest_member_bytes(bytes: &[u8]) -> crate::blake3_content_digest::Blake3ContentDigest {
    crate::blake3_content_digest::Blake3ContentDigest::from_bytes(bytes)
}

#[test]
fn bundle_identity_covers_the_complete_runtime_execution_closure() {
    let members = std::collections::BTreeMap::from([("asp".to_owned(), digest("asp"))]);
    let baseline = RuntimeArtifactBundleBinding::new(
        digest("provider-registration"),
        digest("provider-artifact-set"),
        digest("evaluator-policy"),
        digest("evaluator-abi"),
        digest("schema-bundle"),
    );
    let baseline_digest = runtime_artifact_bound_bundle_digest(&members, &baseline);

    for changed in [
        RuntimeArtifactBundleBinding::new(
            digest("provider-registration-v2"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("evaluator-abi"),
            digest("schema-bundle"),
        ),
        RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set-v2"),
            digest("evaluator-policy"),
            digest("evaluator-abi"),
            digest("schema-bundle"),
        ),
        RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy-v2"),
            digest("evaluator-abi"),
            digest("schema-bundle"),
        ),
        RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("evaluator-abi-v2"),
            digest("schema-bundle"),
        ),
        RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("evaluator-abi"),
            digest("schema-bundle-v2"),
        ),
    ] {
        assert_ne!(
            runtime_artifact_bound_bundle_digest(&members, &changed),
            baseline_digest,
            "every execution-closure leaf must participate in Runtime bundle identity"
        );
    }
}

#[tokio::test]
async fn bound_bundle_admission_rejects_an_unbound_members_only_manifest() {
    let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
    let candidate = temporary.path().join("candidate");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let executable = candidate.join("asp");
    std::fs::write(&executable, b"runtime executable").expect("runtime executable");
    let members = std::collections::BTreeMap::from([(
        "asp".to_owned(),
        runtime_artifact_candidate_digest(&executable)
            .await
            .expect("member digest"),
    )]);
    let unbound_digest = runtime_artifact_bundle_digest(&members);
    std::fs::write(
        candidate.join("bundle.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 1,
            "bundleDigest": unbound_digest,
            "members": members,
        }))
        .expect("unbound manifest bytes"),
    )
    .expect("unbound manifest");

    let error = verify_runtime_artifact_bound_bundle(&candidate)
        .await
        .expect_err("members-only bundles must not enter bound Runtime admission");
    assert!(error.contains("reasonKind=runtime-bundle-binding-missing"));
}

#[tokio::test]
async fn bound_bundle_admission_returns_the_exact_execution_closure() {
    let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
    let candidate = temporary.path().join("candidate");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let executable = candidate.join("asp");
    std::fs::write(&executable, b"runtime executable").expect("runtime executable");
    let mut members = std::collections::BTreeMap::from([(
        "asp".to_owned(),
        runtime_artifact_candidate_digest(&executable)
            .await
            .expect("member digest"),
    )]);
    let binding = write_valid_closure(&candidate, &mut members);
    let bundle_digest =
        stage_runtime_artifact_bound_bundle_manifest(&candidate, &members, &binding)
            .expect("bound manifest");

    let admitted = verify_runtime_artifact_bound_bundle(&candidate)
        .await
        .expect("bound bundle admission");
    assert_eq!(admitted.bundle_digest(), &bundle_digest);
    assert_eq!(
        admitted.execution_binding().provider_registration_digest(),
        binding.provider_registration_digest()
    );
    assert_eq!(admitted.member_path("asp"), Some(executable));
}

#[tokio::test]
async fn bound_bundle_rejects_semantically_forged_closure_with_matching_digests() {
    let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
    let candidate = temporary.path().join("candidate");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let executable = candidate.join("asp");
    std::fs::write(&executable, b"runtime executable").expect("runtime executable");
    let mut members = std::collections::BTreeMap::from([(
        "asp".to_owned(),
        runtime_artifact_candidate_digest(&executable)
            .await
            .expect("member digest"),
    )]);
    write_valid_closure(&candidate, &mut members);
    let registration_path = candidate.join("provider-registration.json");
    let mut registration: crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosureMember<
        crate::runtime_artifact_execution_closure::ProviderRegistrationClosureEntry,
    > = serde_json::from_slice(
        &std::fs::read(&registration_path).expect("registration member"),
    )
    .expect("registration JSON");
    registration.entries[0].language_id = "python".into();
    let registration_bytes = serde_json::to_vec(&registration).expect("forged registration");
    std::fs::write(&registration_path, &registration_bytes).expect("forged member");
    members.insert(
        "provider-registration.json".into(),
        digest_member_bytes(&registration_bytes),
    );
    let binding = RuntimeArtifactBundleBinding::new(
        members["provider-registration.json"].clone(),
        members["provider-artifact-set"].clone(),
        members["evaluator-policy.json"].clone(),
        members["evaluator-abi.json"].clone(),
        members["schema-bundle.json"].clone(),
    );
    stage_runtime_artifact_bound_bundle_manifest(&candidate, &members, &binding)
        .expect("self-consistent forged manifest");

    let error = verify_runtime_artifact_bound_bundle(&candidate)
        .await
        .expect_err("self-consistent bytes cannot forge the built-in registration authority");
    assert!(
        error.contains("runtime-execution-closure-provider-registration-drift"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn bound_bundle_admission_rejects_a_tampered_execution_closure() {
    let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
    let candidate = temporary.path().join("candidate");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let executable = candidate.join("asp");
    std::fs::write(&executable, b"runtime executable").expect("runtime executable");
    let members = std::collections::BTreeMap::from([(
        "asp".to_owned(),
        runtime_artifact_candidate_digest(&executable)
            .await
            .expect("member digest"),
    )]);
    let admitted_binding = RuntimeArtifactBundleBinding::new(
        digest("provider-registration"),
        digest("provider-artifact-set"),
        digest("evaluator-policy"),
        digest("evaluator-abi"),
        digest("schema-bundle"),
    );
    let bundle_digest = runtime_artifact_bound_bundle_digest(&members, &admitted_binding);
    let tampered_binding = RuntimeArtifactBundleBinding::new(
        digest("provider-registration-v2"),
        digest("provider-artifact-set"),
        digest("evaluator-policy"),
        digest("evaluator-abi"),
        digest("schema-bundle"),
    );
    std::fs::write(
        candidate.join("bundle.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 2,
            "bundleDigest": bundle_digest,
            "members": members,
            "executionBinding": tampered_binding,
        }))
        .expect("tampered manifest bytes"),
    )
    .expect("tampered manifest");

    let error = verify_runtime_artifact_bound_bundle(&candidate)
        .await
        .expect_err("a changed closure cannot reuse the prior bundle identity");
    assert!(error.contains("reasonKind=runtime-bundle-binding-digest-mismatch"));
}

#[tokio::test]
async fn bound_bundle_admission_requires_materialized_closure_members() {
    let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
    let candidate = temporary.path().join("candidate");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let executable = candidate.join("asp");
    std::fs::write(&executable, b"runtime executable").expect("runtime executable");
    let members = std::collections::BTreeMap::from([(
        "asp".to_owned(),
        runtime_artifact_candidate_digest(&executable)
            .await
            .expect("member digest"),
    )]);
    let binding = RuntimeArtifactBundleBinding::new(
        digest("provider-registration"),
        digest("provider-artifact-set"),
        digest("evaluator-policy"),
        digest("evaluator-abi"),
        digest("schema-bundle"),
    );
    let bundle_digest = runtime_artifact_bound_bundle_digest(&members, &binding);
    std::fs::write(
        candidate.join("bundle.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 2,
            "bundleDigest": bundle_digest,
            "members": members,
            "executionBinding": binding,
        }))
        .expect("manifest bytes"),
    )
    .expect("manifest");

    let error = verify_runtime_artifact_bound_bundle(&candidate)
        .await
        .expect_err("digest-only closure leaves cannot serve as Runtime content");
    assert!(error.contains("reasonKind=runtime-bundle-binding-member-missing"));
}
