use super::{
    LanguageSchemaClosureEntry, NamedRuntimeDigestClosureEntry, ProviderArtifactClosureEntry,
    ProviderRegistrationClosureEntry, RuntimeArtifactExecutionClosure,
    RuntimeArtifactExecutionClosureMember, RuntimeArtifactExecutionClosureMemberKind,
};
use crate::blake3_content_digest::Blake3ContentDigest;

fn digest(byte: u8) -> Blake3ContentDigest {
    Blake3ContentDigest::from_bytes(&[byte])
}

fn closure() -> RuntimeArtifactExecutionClosure {
    RuntimeArtifactExecutionClosure {
        provider_registration: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
            vec![ProviderRegistrationClosureEntry {
                provider_id: "asp-rust".into(),
                language_id: "rust".into(),
                registration_digest: digest(1),
                artifact_member: "asp-rust".into(),
            }],
        ),
        provider_artifact_set: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
            vec![ProviderArtifactClosureEntry {
                provider_id: "asp-rust".into(),
                artifact_member: "asp-rust".into(),
                artifact_digest: digest(2),
            }],
        ),
        evaluator_policy: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
            vec![NamedRuntimeDigestClosureEntry {
                id: "query-admission".into(),
                digest: digest(4),
            }],
        ),
        evaluator_abi: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
            vec![NamedRuntimeDigestClosureEntry {
                id: "query-playbook-v1".into(),
                digest: digest(5),
            }],
        ),
        schema_bundle: RuntimeArtifactExecutionClosureMember::new(
            RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
            vec![LanguageSchemaClosureEntry {
                language_id: "rust".into(),
                schema_digest: digest(6),
            }],
        ),
    }
}

#[test]
fn complete_closure_mints_the_five_member_bundle_binding() {
    let closure = closure();
    closure.validate().expect("complete closure");
    assert_eq!(closure.materialized_members().unwrap().len(), 5);
    assert_eq!(closure.binding().unwrap(), closure.binding().unwrap());
}

#[test]
fn provider_registration_without_its_artifact_is_rejected() {
    let mut closure = closure();
    closure.provider_artifact_set.entries[0].provider_id = "asp-python".into();
    assert_eq!(
        closure.validate(),
        Err("reasonKind=runtime-execution-closure-provider-coverage-mismatch".to_owned())
    );
}

#[test]
fn explicit_empty_provider_pair_is_a_valid_bootstrap_closure() {
    let mut closure = closure();
    closure.provider_registration.entries.clear();
    closure.provider_artifact_set.entries.clear();
    closure
        .validate()
        .expect("empty provider bootstrap closure");
    assert_eq!(closure.materialized_members().unwrap().len(), 5);
}

#[test]
fn activation_sequence_cannot_fill_an_empty_member() {
    let mut closure = closure();
    closure.evaluator_abi.entries.clear();
    assert_eq!(
        closure.validate(),
        Err("reasonKind=runtime-execution-closure-member-invalid".to_owned())
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
