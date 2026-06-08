use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, RuntimeProviderHealthStatus, builtin_provider_manifests,
    provider_manifest_digest,
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    LanguageId, ProviderExecution, ProviderId, ProviderRegistrySnapshot, ResolvedProvider,
    RuntimeProfileStatus,
};

#[test]
fn runtime_profile_status_preserves_receipt_labels() {
    assert_eq!(RuntimeProfileStatus::Available.as_str(), "available");
    assert_eq!(RuntimeProfileStatus::Missing.as_str(), "missing");
    assert_eq!(RuntimeProfileStatus::Unexecutable.as_str(), "unexecutable");
}

#[test]
fn runtime_profile_status_maps_from_hook_health_status() {
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Available),
        RuntimeProfileStatus::Available
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Missing),
        RuntimeProfileStatus::Missing
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Unexecutable),
        RuntimeProfileStatus::Unexecutable
    );
}

#[test]
fn activation_provider_prefix_takes_precedence_over_runtime_profile_argv() {
    let provider = ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: vec!["./.bin/rs-harness".to_string()],
        runtime_command_argv: Some(vec!["/opt/homebrew/bin/rs-harness".to_string()]),
        runtime_profile_status: Some(RuntimeProfileStatus::Available),
        package_roots: vec![".".to_string()],
    };

    assert_eq!(
        provider.command_prefix(),
        vec!["./.bin/rs-harness".to_string()]
    );
    assert_eq!(provider.runtime_command_prefix(), None);
}

#[test]
fn activation_snapshot_skips_runtime_profile_when_prefix_is_present() {
    let root = temp_root("activation-prefix-snapshot");
    let activation_path = root.join("activation.json");
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "python")
        .expect("python manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation = json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "execution": manifest.execution,
            "providerCommandPrefix": ["missing-python-provider-prefix"],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": manifest.source.default_source_roots,
                "configFiles": manifest.source.default_config_files,
                "sourceExtensions": manifest.source.default_extensions,
                "ignoredPathPrefixes": manifest.source.default_ignored_path_prefixes
            }
        }]
    });
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("activation json"),
    )
    .expect("write activation");

    let snapshot = ProviderRegistrySnapshot::load_from_path(&activation_path).expect("snapshot");
    let provider = snapshot
        .provider_for_language(&LanguageId::from("python"))
        .expect("python provider");

    assert_eq!(
        provider.command_prefix(),
        vec!["missing-python-provider-prefix".to_string()]
    );
    assert_eq!(provider.runtime_command_prefix(), None);
    assert_eq!(provider.runtime_profile_status, None);
    std::fs::remove_dir_all(root).expect("remove temp root");
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-core-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
