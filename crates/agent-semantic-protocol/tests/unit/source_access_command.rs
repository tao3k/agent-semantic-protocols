#![allow(dead_code)]

#[path = "../../src/command/source_access.rs"]
mod source_access;

use agent_semantic_hook::source_access::SourceAccessDecisionKind;
use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
use serde_json::json;
use std::path::{Path, PathBuf};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn temp_root() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("asp-source-access-test-{suffix}"));
    std::fs::create_dir_all(&root).expect("temp root");
    root
}

fn write_activation(root: &Path, language_id: &str) -> PathBuf {
    write_activation_specs(root, &[(language_id, &["."])])
}

fn write_activation_with_languages(root: &Path, language_ids: &[&str]) -> PathBuf {
    let specs: Vec<_> = language_ids
        .iter()
        .map(|language_id| (*language_id, &["."][..]))
        .collect();
    write_activation_specs(root, &specs)
}

fn write_activation_with_package_roots(
    root: &Path,
    language_id: &str,
    package_roots: &[&str],
) -> PathBuf {
    write_activation_specs(root, &[(language_id, package_roots)])
}

fn write_activation_specs(root: &Path, specs: &[(&str, &[&str])]) -> PathBuf {
    let activation_path = root.join("activation.json");
    let manifests = builtin_provider_manifests();
    let providers: Vec<_> = specs
        .iter()
        .map(|(language_id, package_roots)| {
            let manifest = manifests
                .iter()
                .find(|manifest| manifest.language_id == *language_id)
                .unwrap_or_else(|| panic!("manifest for {language_id}"));
            let manifest_digest = provider_manifest_digest(manifest).expect("manifest digest");
            json!({
                "manifestId": manifest.manifest_id,
                "manifestDigest": manifest_digest,
                "languageId": manifest.language_id,
                "providerId": manifest.provider_id,
                "binary": manifest.binary,
                "providerCommandPrefix": [],
                "coverage": {
                    "packageRoots": package_roots,
                    "sourceRoots": manifest.source.default_source_roots,
                    "configFiles": manifest.source.default_config_files,
                    "sourceExtensions": manifest.source.default_extensions,
                    "ignoredPathPrefixes": manifest.source.default_ignored_path_prefixes
                }
            })
        })
        .collect();
    let activation = json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {"runtime": "agent-semantic-hook", "version": "test"},
        "providers": providers
    });
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("json"),
    )
    .expect("activation");
    activation_path
}

#[test]
fn read_file_command_returns_hard_deny_for_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "--rpc-method",
        "fs/readFile",
        "src/cli/agent-hooks.ts",
    ]))
    .expect("decision")
    .expect("source decision");
    let value = serde_json::to_value(&decision).expect("json");

    assert_eq!(decision.decision, SourceAccessDecisionKind::Deny);
    assert_eq!(value["providerId"], "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        args(&[
            "asp",
            "typescript",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            ".",
        ])
    );
}

#[test]
fn shell_egress_command_returns_suppress_for_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--command",
        "sed -n '1,120p' src/cli/agent-hooks.ts",
        "--output-digest",
        "sha256:source-like-output",
        "src/cli/agent-hooks.ts",
    ]))
    .expect("decision")
    .expect("source decision");
    let value = serde_json::to_value(&decision).expect("json");

    assert_eq!(decision.decision, SourceAccessDecisionKind::Suppress);
    assert!(decision.source_bytes_returned);
    assert!(!decision.model_visible_bytes_returned);
    assert_eq!(value["providerId"], "ts-harness");
}

#[test]
fn source_access_command_returns_none_for_non_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "README.md",
    ]))
    .expect("decision");

    assert!(decision.is_none());
}

#[test]
fn read_file_command_selects_provider_by_source_extension() {
    let root = temp_root();
    let activation = write_activation_with_languages(&root, &["rust", "typescript"]);
    let rust_decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/lib.rs",
    ]))
    .expect("decision")
    .expect("rust decision");
    let typescript_decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/index.ts",
    ]))
    .expect("decision")
    .expect("typescript decision");
    let rust_value = serde_json::to_value(&rust_decision).expect("json");
    let typescript_value = serde_json::to_value(&typescript_decision).expect("json");

    assert_eq!(rust_value["providerId"], "rs-harness");
    assert_eq!(rust_decision.routes[0].argv[1], "rust");
    assert_eq!(typescript_value["providerId"], "ts-harness");
    assert_eq!(typescript_decision.routes[0].argv[1], "typescript");
}

#[test]
fn read_file_command_rewrites_selector_to_package_root() {
    let root = temp_root();
    let activation =
        write_activation_with_package_roots(&root, "rust", &["packages/rust-crate", "."]);
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "packages/rust-crate/src/lib.rs",
    ]))
    .expect("decision")
    .expect("source decision");

    assert_eq!(
        decision.routes[0].argv,
        args(&[
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs",
            "--code",
            "packages/rust-crate",
        ])
    );
}

#[test]
fn source_access_command_requires_explicit_activation() {
    let error =
        source_access::source_access_decision_from_args(&args(&["read-file", "src/lib.rs"]))
            .expect_err("missing activation should fail");

    assert!(error.contains("requires --activation"));
}

#[test]
fn source_access_command_rejects_unknown_flags_and_extra_paths() {
    let root = temp_root();
    let activation = write_activation(&root, "rust");
    let unknown = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "--mcp-resource",
        "src/lib.rs",
    ]))
    .expect_err("unknown flag should fail");
    let extra_paths = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/lib.rs",
        "src/main.rs",
    ]))
    .expect_err("extra paths should fail");

    assert!(unknown.contains("unknown source-access flag"));
    assert!(extra_paths.contains("accepts exactly one path"));
}

#[test]
fn shell_egress_command_requires_command_and_output_digest() {
    let root = temp_root();
    let activation = write_activation(&root, "rust");
    let missing_command = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--output-digest",
        "sha256:source-like-output",
        "src/lib.rs",
    ]))
    .expect_err("missing command should fail");
    let missing_digest = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--command",
        "sed -n '1,120p' src/lib.rs",
        "src/lib.rs",
    ]))
    .expect_err("missing output digest should fail");

    assert!(missing_command.contains("requires --command"));
    assert!(missing_digest.contains("requires --output-digest"));
}
