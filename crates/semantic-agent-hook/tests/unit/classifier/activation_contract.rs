use semantic_agent_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION,
    ProviderManifest, parse_activation, provider_manifest_digest,
};
use serde_json::{Value, json};

#[test]
fn activation_protocol_identity_is_validated() {
    let manifest = provider_manifest();
    let digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let mut activation = activation_value(&digest);
    activation["schemaId"] = json!("agent.semantic-protocols.wrong-activation");

    let error = parse_activation(&activation.to_string(), &[manifest]).unwrap_err();

    assert!(format!("{error:?}").contains("schemaId"));
}

#[test]
fn provider_manifest_protocol_identity_is_validated() {
    let mut manifest = provider_manifest_value();
    manifest["schemaId"] = json!("agent.semantic-protocols.wrong-provider-manifest");
    let manifest: ProviderManifest = serde_json::from_value(manifest).expect("manifest shape");
    let digest = provider_manifest_digest(&manifest).expect("manifest digest");

    let error = parse_activation(&activation_value(&digest).to_string(), &[manifest]).unwrap_err();

    assert!(format!("{error:?}").contains("schemaId"));
}

#[test]
fn provider_manifest_rejects_legacy_command_text() {
    let mut manifest = provider_manifest_value();
    manifest["routes"]["prime"]["text"] = json!("ts-harness search prime .");

    let error = serde_json::from_value::<ProviderManifest>(manifest).unwrap_err();

    assert!(error.to_string().contains("unknown field `text`"));
}

#[test]
fn provider_manifest_rejects_null_stdin_mode() {
    let mut manifest = provider_manifest_value();
    manifest["routes"]["prime"]["stdinMode"] = json!(null);

    let error = serde_json::from_value::<ProviderManifest>(manifest).unwrap_err();

    assert!(error.to_string().contains("stdinMode must be omitted"));
}

#[test]
fn activation_rejects_manifest_digest_drift() {
    let manifest = provider_manifest();
    let activation =
        activation_value("sha256:0000000000000000000000000000000000000000000000000000000000000000");

    let error = parse_activation(&activation.to_string(), &[manifest]).unwrap_err();

    assert!(format!("{error:?}").contains("manifest digest mismatch"));
}

#[test]
fn activation_resolves_provider_manifest_and_project_coverage() {
    let manifest = provider_manifest();
    let digest = provider_manifest_digest(&manifest).expect("manifest digest");

    let runtime = parse_activation(&activation_value(&digest).to_string(), &[manifest])
        .expect("activation resolves");

    assert_eq!(runtime.project_root, ".");
    assert_eq!(runtime.providers.len(), 1);
    assert_eq!(runtime.providers[0].language_id, "typescript");
    assert_eq!(runtime.providers[0].provider_id, "ts-harness");
    assert_eq!(runtime.providers[0].source_roots, ["src", "tests"]);
    assert_eq!(
        runtime.providers[0].routes.guide.as_ref().unwrap().argv,
        ["ts-harness", "agent", "guide", "."]
    );
}

fn provider_manifest() -> ProviderManifest {
    serde_json::from_value(provider_manifest_value()).expect("provider manifest")
}

fn provider_manifest_value() -> Value {
    json!({
        "schemaId": PROVIDER_MANIFEST_SCHEMA_ID,
        "schemaVersion": PROVIDER_MANIFEST_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "manifestId": "agent.semantic-protocols.languages.typescript.ts-harness",
        "manifestVersion": "v1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "binary": "ts-harness",
        "source": {
            "defaultExtensions": [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
            "defaultConfigFiles": ["package.json", "tsconfig.json"],
            "defaultSourceRoots": ["src", "tests"],
            "defaultIgnoredPathPrefixes": ["node_modules", "dist"]
        },
        "policy": {
            "directSourceRead": "block",
            "bulkSourceDump": "block",
            "rawSourceSearch": "block",
            "agentSearchJson": "block"
        },
        "routes": {
            "prime": {"argv": ["ts-harness", "search", "prime", "."]},
            "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "."]},
            "fzf": {"argv": ["ts-harness", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "query": {"argv": ["ts-harness", "search", "query", "--from-hook", "direct-source-read", "--selector", "{selector}", "{termArgs}", "--surface", "owner,tests", "--view", "seeds", "."]},
            "ingest": {
                "argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."],
                "stdinMode": "pipe-candidates"
            },
            "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]},
            "guide": {"argv": ["ts-harness", "agent", "guide", "."]}
        }
    })
}

fn activation_value(manifest_digest: &str) -> Value {
    json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {
            "runtime": "semantic-agent-hook",
            "version": "0.1.0"
        },
        "providers": [{
            "manifestId": "agent.semantic-protocols.languages.typescript.ts-harness",
            "manifestDigest": manifest_digest,
            "languageId": "typescript",
            "providerId": "ts-harness",
            "binary": "ts-harness",
            "providerCommandPrefix": ["ts-harness"],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": ["src", "tests"],
                "configFiles": ["package.json", "tsconfig.json"],
                "sourceExtensions": [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
                "ignoredPathPrefixes": ["node_modules", "dist"]
            }
        }]
    })
}
