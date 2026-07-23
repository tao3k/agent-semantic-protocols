use std::path::Path;

use agent_semantic_rust_policy_types::{
    DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID, DownstreamPolicyReceipt, HarnessExecution,
    MEMBER_POLICY_REGISTRY_SCHEMA_ID, MemberPolicy, MemberPolicyRegistry, ReceiptPackage,
    SCHEMA_VERSION, SourceSnapshot, canonical_json_digest,
};
use agent_semantic_rust_policy_verifier::VerificationInput;

pub(super) fn member() -> MemberPolicy {
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

pub(super) fn registry() -> MemberPolicyRegistry {
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

pub(super) fn source_snapshot() -> SourceSnapshot {
    SourceSnapshot {
        digest: format!("blake3:{}", "0".repeat(64)),
        file_count: 3,
        byte_count: 128,
    }
}

pub(super) fn receipt(registry: &MemberPolicyRegistry) -> DownstreamPolicyReceipt {
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

pub(super) fn verification_input<'a>(snapshot: &'a SourceSnapshot) -> VerificationInput<'a> {
    VerificationInput {
        package_name: "demo",
        manifest_dir: Path::new("/workspace/crates/demo"),
        workspace_root: Path::new("/workspace"),
        observed_source_snapshot: snapshot,
    }
}
