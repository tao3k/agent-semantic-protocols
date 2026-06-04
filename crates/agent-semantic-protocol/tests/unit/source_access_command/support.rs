use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
use serde_json::json;
use std::path::{Path, PathBuf};

pub(super) fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

pub(super) fn temp_root() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("asp-source-access-test-{suffix}"));
    std::fs::create_dir_all(&root).expect("temp root");
    root
}

pub(super) fn write_activation(root: &Path, language_id: &str) -> PathBuf {
    write_activation_specs(root, &[(language_id, &["."])])
}

pub(super) fn write_activation_with_languages(root: &Path, language_ids: &[&str]) -> PathBuf {
    let specs: Vec<_> = language_ids
        .iter()
        .map(|language_id| (*language_id, &["."][..]))
        .collect();
    write_activation_specs(root, &specs)
}

pub(super) fn write_activation_with_package_roots(
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
