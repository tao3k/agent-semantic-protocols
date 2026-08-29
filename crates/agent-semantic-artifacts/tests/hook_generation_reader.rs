#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use agent_semantic_artifacts::hook_generation::{
    HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
    read_current_hook_generation, recover_missing_hook_generation,
};

fn hook_binary(root: &Path) -> std::path::PathBuf {
    let path = root.join("fixture-hook-binary");
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("Hook binary");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("Hook binary mode");
    path
}

fn publish(root: &Path) {
    let hook_binary = hook_binary(root);
    let prepared = prepare_hook_generation(
        root,
        HookGenerationCandidate {
            hook_binary: &hook_binary,
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
    assert!(receipt.hook_binary_path.is_file());
    assert!(receipt.generation_path.is_file());
    assert_eq!(
        receipt.hook_binary_path.parent(),
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
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("current receipt bytes"))
            .expect("current receipt json");
    value["schemaVersion"] = serde_json::json!(2);
    std::fs::write(
        &receipt_path,
        serde_json::to_vec(&value).expect("future receipt json"),
    )
    .expect("future receipt");
    assert!(read_current_hook_generation(future_root.path()).is_err());
}

#[test]
fn current_hook_generation_reader_rejects_tampered_hook_binary() {
    let root = tempfile::tempdir().expect("state home");
    publish(root.path());
    let receipt = read_current_hook_generation(root.path())
        .expect("read current generation")
        .expect("current generation");
    std::fs::write(&receipt.hook_binary_path, b"#!/bin/sh\nexit 23\n").expect("tamper Hook binary");

    let error = read_current_hook_generation(root.path())
        .expect_err("Hook binary digest mismatch must fail closed");
    assert!(
        error.contains("does not match immutable candidate bytes"),
        "{error}"
    );
}

#[test]
fn missing_current_recovers_once_from_a_verified_candidate() {
    let root = tempfile::tempdir().expect("state home");
    let hook_binary = hook_binary(root.path());
    let publication = recover_missing_hook_generation(
        root.path(),
        HookGenerationCandidate {
            hook_binary: &hook_binary,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect("missing current recovery");
    assert_eq!(publication.retained_generation_count, 1);
    assert!(
        read_current_hook_generation(root.path())
            .expect("read recovered current")
            .is_some()
    );
    let second = recover_missing_hook_generation(
        root.path(),
        HookGenerationCandidate {
            hook_binary: &hook_binary,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect_err("recovery must not replace an existing current");
    assert!(second.contains("already published"), "{second}");
}

#[test]
fn invalid_current_is_not_replaced_by_recovery() {
    let root = tempfile::tempdir().expect("state home");
    publish(root.path());
    let current = root.path().join("hooks/current");
    std::fs::remove_file(&current).expect("remove current for malformed link");
    std::fs::write(&current, b"not-a-link").expect("write malformed current");
    let hook_binary = hook_binary(root.path());
    let error = recover_missing_hook_generation(
        root.path(),
        HookGenerationCandidate {
            hook_binary: &hook_binary,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect_err("malformed current must fail closed");
    assert!(
        error.contains("invalid; recovery preserves fail-closed state"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(&current).expect("current remains"),
        b"not-a-link"
    );
}

#[test]
fn same_generation_tamper_is_rejected_before_current_switch() {
    let root = tempfile::tempdir().expect("state home");
    let hook_binary = hook_binary(root.path());
    let first = prepare_hook_generation(
        root.path(),
        HookGenerationCandidate {
            hook_binary: &hook_binary,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect("prepare first");
    let digest = first.receipt.generation_digest.clone();
    commit_hook_generation(root.path(), &first).expect("commit first");
    let current_before = std::fs::read_link(root.path().join("hooks/current")).expect("current");
    let tampered = root
        .path()
        .join("hooks/generations/blake3-256")
        .join(digest.trim_start_matches("blake3-256:"))
        .join("registry.bin");
    std::fs::write(&tampered, b"tampered").expect("tamper same generation");

    let duplicate = prepare_hook_generation(
        root.path(),
        HookGenerationCandidate {
            hook_binary: &hook_binary,
            config: b"schemaVersion = 1\n",
            compiled_matcher: br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:fixture","rules":[]}"#,
            registry: b"registry-fixture",
        },
    )
    .expect("prepare duplicate");
    let error = commit_hook_generation(root.path(), &duplicate)
        .expect_err("tampered duplicate must fail closed");
    assert!(
        error.contains("does not match prepared immutable bytes"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_link(root.path().join("hooks/current")).expect("current"),
        current_before
    );
}
