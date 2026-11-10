// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{WorkspaceGenerationAdmissionBindingV1, candidate_is_restart_verifiable};

fn digest(prefix: &str, byte: char) -> String {
    format!("{prefix}{}", byte.to_string().repeat(64))
}

fn binding() -> WorkspaceGenerationAdmissionBindingV1 {
    WorkspaceGenerationAdmissionBindingV1::new(
        "workspace-v1".to_owned(),
        std::path::PathBuf::from("/tmp/workspace-v1"),
        git_candidate('a'),
        digest("blake3-256:", 'b'),
        digest("blake3-256:", 'c'),
    )
    .expect("valid V1 binding")
}

fn git_candidate(
    digest: char,
) -> crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
    crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: format!("blake3:{}", digest.to_string().repeat(64)),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest: format!("blake3:{}", digest.to_string().repeat(64)),
    }
}

#[test]
fn binding_serialization_stays_v1_and_contains_the_exact_candidate() {
    let value = serde_json::to_value(binding()).expect("serialize V1 binding");
    assert_eq!(value["schemaVersion"], "1");
    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.workspace-generation-admission-binding"
    );
    assert_eq!(
        value["candidate"]["candidateGeneration"]["digest"],
        format!("blake3:{}", "a".repeat(64))
    );
}

#[test]
fn reuse_requires_every_scope_candidate_and_generation_identity() {
    let binding = binding();
    let root = std::path::Path::new("/tmp/workspace-v1");
    let candidate = git_candidate('a');
    assert!(binding.admits_identity(
        "workspace-v1",
        root,
        &candidate,
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'c'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-v1",
        root,
        &git_candidate('d'),
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'c'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-v1",
        root,
        &candidate,
        &digest("blake3-256:", 'e'),
        &digest("blake3-256:", 'c'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-v1",
        std::path::Path::new("/tmp/relocated-workspace-v1"),
        &candidate,
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'c'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-v1",
        root,
        &candidate,
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'f'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-other",
        root,
        &candidate,
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'c'),
        "workspace-v1",
    ));
    assert!(!binding.admits_identity(
        "workspace-v1",
        root,
        &candidate,
        &digest("blake3-256:", 'b'),
        &digest("blake3-256:", 'c'),
        "workspace-other",
    ));
}

#[tokio::test]
async fn atomic_sidecar_round_trip_succeeds_and_corruption_fails_closed() {
    let directory = tempfile::tempdir().expect("generation directory");
    super::publish(directory.path(), &binding())
        .await
        .expect("publish V1 binding");
    assert_eq!(
        super::read(directory.path())
            .await
            .expect("read V1 binding"),
        binding()
    );

    tokio::fs::write(directory.path().join(super::FILE_NAME), b"{truncated")
        .await
        .expect("corrupt V1 binding");
    assert!(
        super::read(directory.path())
            .await
            .expect_err("corrupt binding must not be admitted")
            .contains("decode workspace generation admission binding")
    );
}

#[tokio::test]
async fn publication_without_candidate_authority_revokes_an_existing_binding() {
    let directory = tempfile::tempdir().expect("generation directory");
    super::publish(directory.path(), &binding())
        .await
        .expect("publish V1 binding");
    super::invalidate(directory.path())
        .await
        .expect("revoke V1 binding");
    assert!(
        super::read(directory.path())
            .await
            .expect_err("revoked binding must not remain readable")
            .contains("read workspace generation admission binding")
    );
}

#[test]
fn only_git_content_candidates_are_restart_verifiable() {
    assert!(candidate_is_restart_verifiable(&git_candidate('d')));
    let runtime = crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity::for_runtime_admission(
        "project-v1",
        "workspace-v1",
        None,
    )
    .expect("runtime candidate");
    assert!(!candidate_is_restart_verifiable(&runtime));
}
