#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use agent_semantic_artifacts::hook_generation::{
    HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
    read_current_hook_generation,
};

fn evaluator(root: &Path) -> std::path::PathBuf {
    let path = root.join("fixture-evaluator");
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("evaluator");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("evaluator mode");
    path
}

fn publish(root: &Path) {
    let evaluator = evaluator(root);
    let prepared = prepare_hook_generation(
        root,
        HookGenerationCandidate {
            evaluator_binary: &evaluator,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect("prepare generation");
    commit_hook_generation(root, &prepared).expect("commit generation");
}

#[test]
fn current_hook_generation_reader_returns_none_when_unpublished() {
    let root = tempfile::tempdir().expect("state home");
    assert!(
        read_current_hook_generation(root.path())
            .expect("read unpublished generation")
            .is_none()
    );
}

#[test]
fn current_hook_generation_reader_validates_complete_immutable_candidate() {
    let root = tempfile::tempdir().expect("state home");
    publish(root.path());
    let receipt = read_current_hook_generation(root.path())
        .expect("read current generation")
        .expect("current generation");
    assert_eq!(receipt.schema_version, 1);
    assert!(receipt.evaluator_path.is_file());
    assert!(receipt.generation_path.is_file());
    assert_eq!(
        receipt.evaluator_path.parent(),
        receipt.generation_path.parent()
    );
}

#[test]
fn current_hook_generation_reader_fails_closed_for_malformed_or_future_receipt() {
    let root = tempfile::tempdir().expect("state home");
    publish(root.path());
    let receipt = read_current_hook_generation(root.path())
        .expect("read current generation")
        .expect("current generation");
    let receipt_path = receipt
        .generation_path
        .parent()
        .expect("candidate directory")
        .join("hook-generation-receipt.json");
    std::fs::write(&receipt_path, b"not-json").expect("malformed receipt");
    assert!(read_current_hook_generation(root.path()).is_err());

    let future_root = tempfile::tempdir().expect("future state home");
    publish(future_root.path());
    let receipt = read_current_hook_generation(future_root.path())
        .expect("read replacement generation")
        .expect("replacement generation");
    let receipt_path = receipt
        .generation_path
        .parent()
        .expect("candidate directory")
        .join("hook-generation-receipt.json");
    let mut value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&receipt_path).expect("current receipt bytes"),
    )
    .expect("current receipt json");
    value["schemaVersion"] = serde_json::json!(2);
    std::fs::write(
        &receipt_path,
        serde_json::to_vec(&value).expect("future receipt json"),
    )
    .expect("future receipt");
    assert!(read_current_hook_generation(future_root.path()).is_err());
}
