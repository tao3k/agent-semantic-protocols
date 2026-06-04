use agent_semantic_hook::source_access::{
    SourceAccessAuthorization, SourceAccessDecision, SourceAccessDecisionKind,
    SourceAccessEnforcement, SourceAccessHardFsDenyInput, SourceAccessProviderCapabilityAllowInput,
    SourceAccessShellEgressSuppressedInput, codex_fs_read_file_decision,
    codex_shell_egress_suppression_decision,
};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn hard_fs_deny_serializes_no_source_bytes_returned() {
    let decision = SourceAccessDecision::hard_fs_deny(SourceAccessHardFsDenyInput {
        language_id: "rust".to_string(),
        provider_id: "rs-harness".into(),
        rpc_method: "fs/readFile".to_string(),
        path: "src/lib.rs".to_string(),
        route_argv: vec![
            "asp".into(),
            "rust".into(),
            "query".into(),
            "--from-hook".into(),
            "direct-source-read".into(),
            "--selector".into(),
            "src/lib.rs".into(),
            "--code".into(),
            ".".into(),
        ],
    });
    let value = serde_json::to_value(decision).expect("serializes");

    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.source-access.decision"
    );
    assert_eq!(value["boundary"], "codex-fs-api");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["sourceBytesReturned"], false);
    assert_eq!(value["modelVisibleBytesReturned"], false);
    assert_eq!(value["subject"]["rpcMethod"], "fs/readFile");
    assert_eq!(value["routes"][0]["binary"], "asp");
}

#[test]
fn shell_egress_suppression_records_hidden_subprocess_output() {
    let decision =
        SourceAccessDecision::shell_egress_suppressed(SourceAccessShellEgressSuppressedInput {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".into(),
            command: "sed -n '1,120p' src/lib.rs".to_string(),
            path: "src/lib.rs".to_string(),
            output_digest: "sha256:source-like-output".to_string(),
            route_argv: vec![
                "asp".into(),
                "rust".into(),
                "query".into(),
                "--from-hook".into(),
                "direct-source-read".into(),
                "--selector".into(),
                "src/lib.rs".into(),
                "--code".into(),
                ".".into(),
            ],
        });
    let value = serde_json::to_value(decision).expect("serializes");

    assert_eq!(value["boundary"], "codex-shell-egress");
    assert_eq!(value["enforcement"], "egress");
    assert_eq!(value["decision"], "suppress");
    assert_eq!(value["sourceBytesReturned"], true);
    assert_eq!(value["modelVisibleBytesReturned"], false);
    assert_eq!(
        value["subject"]["outputDigest"],
        "sha256:source-like-output"
    );
}

#[test]
fn provider_capability_allow_keeps_authorization_explicit() {
    let decision =
        SourceAccessDecision::provider_capability_allow(SourceAccessProviderCapabilityAllowInput {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".into(),
            command: "asp rust query --from-hook direct-source-read --selector src/lib.rs --code ."
                .to_string(),
            path: "src/lib.rs".to_string(),
        });

    assert_eq!(decision.decision, SourceAccessDecisionKind::Allow);
    assert_eq!(decision.enforcement, SourceAccessEnforcement::Hard);
    assert_eq!(
        decision.authorization,
        Some(SourceAccessAuthorization::ProviderCapability)
    );

    let value = serde_json::to_value(decision).expect("serializes");
    assert_eq!(
        value,
        json!({
            "schemaId": "agent.semantic-protocols.source-access.decision",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.source-access",
            "protocolVersion": "1",
            "client": "codex",
            "boundary": "codex-tool-action",
            "operation": "read-file",
            "enforcement": "hard",
            "decision": "allow",
            "reasonKind": "provider-authorized",
            "sourceBytesReturned": true,
            "modelVisibleBytesReturned": true,
            "authorization": "provider-capability",
            "languageIds": ["rust"],
            "providerId": "rs-harness",
            "subject": {
                "toolName": "asp",
                "command": "asp rust query --from-hook direct-source-read --selector src/lib.rs --code .",
                "paths": ["src/lib.rs"]
            },
            "message": "provider-capability allowed compact source access."
        })
    );
}

#[test]
fn codex_fs_read_file_policy_denies_activated_source_path() {
    let decision =
        codex_fs_read_file_decision(&registry(), "fs/readFile", "src/cli/agent-hooks.ts")
            .expect("source path is denied");
    let value = serde_json::to_value(decision).expect("serializes");

    assert_eq!(value["boundary"], "codex-fs-api");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["languageIds"], json!(["typescript"]));
    assert_eq!(value["providerId"], "ts-harness");
    assert_eq!(
        value["routes"][0]["argv"],
        json!([
            "asp",
            "typescript",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            "."
        ])
    );
}

#[test]
fn codex_shell_egress_policy_suppresses_activated_source_output() {
    let decision = codex_shell_egress_suppression_decision(
        &registry(),
        "sed -n '1,120p' src/cli/agent-hooks.ts",
        "src/cli/agent-hooks.ts",
        "sha256:source-like-output",
    )
    .expect("source output is suppressed");
    let value = serde_json::to_value(decision).expect("serializes");

    assert_eq!(value["boundary"], "codex-shell-egress");
    assert_eq!(value["decision"], "suppress");
    assert_eq!(value["sourceBytesReturned"], true);
    assert_eq!(value["modelVisibleBytesReturned"], false);
    assert_eq!(value["providerId"], "ts-harness");
}

#[test]
fn codex_source_access_policy_ignores_non_source_path() {
    assert!(codex_fs_read_file_decision(&registry(), "fs/readFile", "README.md").is_none());
    assert!(
        codex_shell_egress_suppression_decision(
            &registry(),
            "sed -n '1,120p' README.md",
            "README.md",
            "sha256:docs-output",
        )
        .is_none()
    );
}
