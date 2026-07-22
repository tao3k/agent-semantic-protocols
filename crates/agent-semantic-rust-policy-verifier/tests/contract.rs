use std::path::Path;

use agent_semantic_rust_policy_types::{
    DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID, DownstreamPolicyReceipt, HarnessExecution,
    MEMBER_POLICY_REGISTRY_SCHEMA_ID, MemberPolicy, MemberPolicyRegistry, ReceiptPackage,
    SCHEMA_VERSION, SourceSnapshot, canonical_json_digest,
};
use agent_semantic_rust_policy_verifier::{
    VerificationInput, prepare_command, verify_receipt_bytes,
};

fn member() -> MemberPolicy {
    MemberPolicy {
        package: "demo".into(),
        package_directory: "crates/demo".into(),
        gate_label: "demo-policy".into(),
        snapshot_excludes: Vec::new(),
        agent_advice_allow_explanation: None,
        cargo_check_advice_allow_explanation: None,
        cargo_test_advice_allow_explanation: None,
        rule_severity_overrides: Vec::new(),
        verification_skills: Vec::new(),
        owners: Vec::new(),
        dependency_baseline_packages: Vec::new(),
    }
}

fn registry() -> MemberPolicyRegistry {
    MemberPolicyRegistry {
        schema_id: MEMBER_POLICY_REGISTRY_SCHEMA_ID.into(),
        schema_version: SCHEMA_VERSION.into(),
        harness_execution: HarnessExecution {
            runner_package: "rust-project-harness-policy-runner".into(),
            runner_version: "0.1.0".into(),
            prepare_command: vec![
                "cargo".into(),
                "run".into(),
                "-p".into(),
                "rust-project-harness-policy-runner".into(),
                "--".into(),
                "prepare".into(),
                "--workspace-root".into(),
                ".".into(),
                "--package".into(),
                "{package}".into(),
            ],
        },
        members: vec![member()],
    }
}

fn source_snapshot() -> SourceSnapshot {
    SourceSnapshot {
        digest: format!("blake3:{}", "0".repeat(64)),
        file_count: 3,
        byte_count: 128,
    }
}

fn receipt(registry: &MemberPolicyRegistry) -> DownstreamPolicyReceipt {
    DownstreamPolicyReceipt {
        schema_id: DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID.into(),
        schema_version: SCHEMA_VERSION.into(),
        package: ReceiptPackage {
            name: "demo".into(),
            directory: "crates/demo".into(),
        },
        gate_label: "demo-policy".into(),
        source_snapshot: source_snapshot(),
        policy_digest: canonical_json_digest(&registry.members[0]).unwrap(),
        execution_command_digest: canonical_json_digest(&registry.harness_execution).unwrap(),
        dependency_baseline_packages: Vec::new(),
        cache_payload_digest: format!("blake3:{}", "2".repeat(64)),
        active_verification_task_count: 0,
        performance_task_count: 0,
        stability_task_count: 0,
        performance_report_obligation: false,
        stability_report_obligation: false,
        report_obligations: Vec::new(),
    }
}

fn verification_input<'a>(snapshot: &'a SourceSnapshot) -> VerificationInput<'a> {
    VerificationInput {
        package_name: "demo",
        manifest_dir: Path::new("/workspace/crates/demo"),
        workspace_root: Path::new("/workspace"),
        observed_source_snapshot: snapshot,
    }
}

#[test]
fn prepare_command_materializes_the_selected_package() {
    let rendered = prepare_command(&registry(), "demo");
    assert!(rendered.contains("--package demo"), "rendered={rendered}");
    assert!(!rendered.contains("{package}"));
}

#[test]
fn malformed_registry_fails_closed_before_receipt_acceptance() {
    let snapshot = source_snapshot();
    let error = verify_receipt_bytes(b"{}", b"{}", verification_input(&snapshot)).unwrap_err();
    assert!(error.contains("registry"));
}

#[test]
fn valid_receipt_is_accepted_and_source_drift_is_rejected() {
    let registry = registry();
    let receipt = receipt(&registry);
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    let receipt_bytes = serde_json::to_vec(&receipt).unwrap();
    let snapshot = source_snapshot();

    let verified = verify_receipt_bytes(
        &registry_bytes,
        &receipt_bytes,
        verification_input(&snapshot),
    )
    .unwrap();
    assert_eq!(verified.package_name, "demo");
    assert_eq!(verified.package_directory, "crates/demo");

    let drifted = SourceSnapshot {
        digest: format!("blake3:{}", "1".repeat(64)),
        ..snapshot
    };
    let error = verify_receipt_bytes(
        &registry_bytes,
        &receipt_bytes,
        verification_input(&drifted),
    )
    .unwrap_err();
    assert!(error.contains("source snapshot"));
}

#[test]
fn policy_and_execution_drift_fail_closed() {
    let registry = registry();
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    let snapshot = source_snapshot();

    let mut policy_drift = receipt(&registry);
    policy_drift.policy_digest = format!("blake3:{}", "3".repeat(64));
    let error = verify_receipt_bytes(
        &registry_bytes,
        &serde_json::to_vec(&policy_drift).unwrap(),
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(error.contains("policy drift"), "error={error}");

    let mut execution_drift = receipt(&registry);
    execution_drift.execution_command_digest = format!("blake3:{}", "4".repeat(64));
    let error = verify_receipt_bytes(
        &registry_bytes,
        &serde_json::to_vec(&execution_drift).unwrap(),
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(error.contains("execution-command drift"), "error={error}");
}

#[test]
fn duplicate_registry_member_is_rejected() {
    let mut registry = registry();
    registry.members.push(member());
    let snapshot = source_snapshot();
    let error = verify_receipt_bytes(
        &serde_json::to_vec(&registry).unwrap(),
        b"{}",
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(
        error.contains("duplicate Rust member policy"),
        "error={error}"
    );
}
