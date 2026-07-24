use agent_semantic_hook::{
    ActivatedProviderConfig, ActivationCoverage, HOOK_ACTIVATION_SCHEMA_ID,
    HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, ProviderManifest,
    builtin_provider_manifests, parse_activation, provider_manifest_digest,
};
use serde_json::{Value, json};

#[test]
fn activation_protocol_identity_is_validated() {
    let manifest = provider_manifest();
    let digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let mut activation = activation_value(&manifest, &digest);
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

    let error = parse_activation(
        &activation_value(&manifest, &digest).to_string(),
        &[manifest],
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("schemaId"));
}

#[test]
fn activation_rejects_manifest_digest_drift() {
    let manifest = provider_manifest();
    let activation = activation_value(
        &manifest,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );

    let error = parse_activation(&activation.to_string(), &[manifest]).unwrap_err();
    let error_text = format!("{error:?}");

    assert!(
        error_text.contains("provider manifest digest drift"),
        "{error_text}"
    );
}

#[test]
fn activation_resolves_provider_manifest_and_project_coverage() {
    let manifest = provider_manifest();
    let digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let expected_source_roots = manifest.source.default_source_roots.clone();
    let expected_guide_argv = agent_semantic_hook::materialize_provider_routes(&manifest)
        .expect("materialize TypeScript routes")
        .guide
        .as_ref()
        .expect("builtin TypeScript guide route")
        .argv
        .clone();

    let runtime = parse_activation(
        &activation_value(&manifest, &digest).to_string(),
        &[manifest],
    )
    .expect("activation resolves");

    assert_eq!(runtime.project_root, ".");
    assert_eq!(runtime.providers.len(), 1);
    assert_eq!(runtime.providers[0].language_id, "typescript");
    assert_eq!(runtime.providers[0].provider_id, "ts-harness");
    assert_eq!(runtime.providers[0].source_roots, expected_source_roots);
    assert_eq!(
        runtime.providers[0].routes.guide.as_ref().unwrap().argv,
        expected_guide_argv
    );
}

fn provider_manifest() -> ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .find(|manifest| {
            manifest.language_id == "typescript" && manifest.provider_id == "ts-harness"
        })
        .expect("builtin TypeScript provider manifest")
}

fn provider_manifest_value() -> Value {
    serde_json::to_value(provider_manifest()).expect("serialize provider manifest")
}

fn activation_value(manifest: &ProviderManifest, manifest_digest: &str) -> Value {
    let routes =
        agent_semantic_hook::materialize_provider_routes(manifest).expect("provider routes");
    let provider = ActivatedProviderConfig {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: manifest_digest.to_string(),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: manifest.binary.clone(),
        execution: manifest.execution,
        provider_command_prefix: vec![manifest.binary.clone()],
        execution_command_digest: "test-execution-command-digest".to_string(),
        search_capabilities: manifest.search_capabilities.clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
        query_pack_descriptor: manifest.query_pack_descriptor.clone(),
        semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
        routes,
        coverage: ActivationCoverage {
            package_roots: vec![".".to_string()],
            source_roots: manifest.source.default_source_roots.clone(),
            config_files: manifest.source.default_config_files.clone(),
            source_extensions: manifest.source.default_extensions.clone(),
            ignored_path_prefixes: manifest.source.default_ignored_path_prefixes.clone(),
        },
    };
    json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "schemaAuthority": agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {
            "runtime": "agent-semantic-hook",
            "version": "0.1.0"
        },
        "providers": [provider]
    })
}
