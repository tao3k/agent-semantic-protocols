use std::collections::BTreeSet;

use agent_semantic_hook::builtin_provider_manifests;
use serde_json::Value;

#[test]
fn every_builtin_language_owns_a_query_pack_descriptor() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-query-pack-descriptor.v2.schema.json"
    ))
    .expect("valid provider query pack descriptor schema");
    let descriptor_version = schema
        .pointer("/properties/descriptorVersion/const")
        .and_then(Value::as_str)
        .expect("query pack descriptor version contract");
    let manifests = builtin_provider_manifests();

    assert!(
        !manifests.is_empty(),
        "builtin provider registry must not be empty"
    );

    let mut provider_identities = BTreeSet::new();
    let mut descriptor_ids = BTreeSet::new();
    for manifest in manifests {
        assert!(
            provider_identities
                .insert((manifest.language_id.clone(), manifest.provider_id.clone(),)),
            "duplicate builtin provider identity language={} provider={}",
            manifest.language_id,
            manifest.provider_id
        );
        let descriptor = &manifest.query_pack_descriptor;
        assert_eq!(descriptor.language_id, manifest.language_id);
        assert_eq!(descriptor.descriptor_version, descriptor_version);
        assert!(descriptor_ids.insert(descriptor.descriptor_id.clone()));
        assert!(!descriptor.recipes.is_empty());
        assert!(descriptor.term_role_overrides.iter().all(|override_| {
            !override_.term.is_empty()
                && matches!(override_.role.as_str(), "context" | "symbol" | "concept")
        }));
        assert!(descriptor.recipes.iter().all(|recipe| {
            !recipe.recipe_id.is_empty()
                && !recipe.trigger.terms.is_empty()
                && matches!(recipe.trigger.r#match.as_str(), "all" | "any")
                && !recipe.clauses.is_empty()
                && recipe.clauses.iter().all(|clause| {
                    !clause.terms.is_empty()
                        && clause
                            .roles
                            .iter()
                            .all(|role| matches!(role.as_str(), "context" | "symbol" | "concept"))
                })
        }));
    }
}

#[test]
fn provider_and_activation_v3_require_query_pack_descriptors() {
    let provider_schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-manifest.v3.schema.json"
    ))
    .expect("valid provider manifest v3 schema");
    assert_eq!(
        provider_schema.pointer("/properties/schemaVersion/const"),
        Some(&Value::String("3".to_string()))
    );
    assert!(
        provider_schema["required"]
            .as_array()
            .expect("provider required fields")
            .iter()
            .any(|field| field == "queryPackDescriptor")
    );
    assert!(
        provider_schema["required"]
            .as_array()
            .expect("provider required fields")
            .iter()
            .any(|field| field == "searchCapabilities")
    );
    let required_capabilities = [
        "ownerItems",
        "semanticFacts",
        "dependencyTopology",
        "dependencyTopologyMetadata",
        "workspaceScope",
    ];
    let provider_capabilities_required =
        provider_schema["$defs"]["providerSearchCapabilities"]["required"]
            .as_array()
            .expect("provider search capability required fields");
    assert!(required_capabilities.iter().all(|required| {
        provider_capabilities_required
            .iter()
            .any(|field| field == required)
    }));
    assert_eq!(
        provider_schema.pointer("/properties/queryPackDescriptor/$ref"),
        Some(&Value::String(
            "provider-query-pack-descriptor.v1.schema.json".to_string()
        ))
    );

    let activation_schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/hook-activation.v3.schema.json"
    ))
    .expect("valid hook activation v3 schema");
    assert_eq!(
        activation_schema.pointer("/properties/schemaVersion/const"),
        Some(&Value::String("3".to_string()))
    );
    assert!(
        activation_schema["$defs"]["activatedProviderConfig"]["required"]
            .as_array()
            .expect("activated provider required fields")
            .iter()
            .any(|field| field == "queryPackDescriptor")
    );
    assert!(
        activation_schema["$defs"]["activatedProviderConfig"]["required"]
            .as_array()
            .expect("activated provider required fields")
            .iter()
            .any(|field| field == "searchCapabilities")
    );
    assert_eq!(
        activation_schema
            .pointer("/$defs/activatedProviderConfig/properties/searchCapabilities/$ref"),
        Some(&Value::String(
            "provider-manifest.v3.schema.json#/$defs/providerSearchCapabilities".to_string()
        ))
    );
    assert_eq!(
        activation_schema
            .pointer("/$defs/activatedProviderConfig/properties/queryPackDescriptor/$ref"),
        Some(&Value::String(
            "provider-query-pack-descriptor.v1.schema.json".to_string()
        ))
    );
}

#[test]
fn registry_v3_descriptors_match_builtin_provider_manifests() {
    let registry: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-language-registry.providers.v3.json"
    ))
    .expect("valid semantic language registry v3");
    let registrations = registry["languages"]
        .as_array()
        .expect("registry languages");
    let manifests = builtin_provider_manifests();
    assert_eq!(registrations.len(), manifests.len());

    for manifest in manifests {
        let registration = registrations
            .iter()
            .find(|registration| {
                registration["languageId"] == manifest.language_id
                    && registration["providerId"] == manifest.provider_id
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing registry entry for language={} provider={}",
                    manifest.language_id, manifest.provider_id
                )
            });
        let registry_descriptor: agent_semantic_hook::ProviderQueryPackDescriptor =
            serde_json::from_value(registration["queryPackDescriptor"].clone())
                .expect("parse registry query-pack descriptor");
        assert_eq!(registry_descriptor, manifest.query_pack_descriptor);
    }
}

#[test]
fn activation_rejects_search_capabilities_drift() {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .next()
        .expect("a builtin provider must exist");
    let manifest_digest =
        agent_semantic_hook::provider_manifest_digest(&manifest).expect("digest provider manifest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let mut activation = agent_semantic_hook::HookActivation {
        schema_id: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: agent_semantic_hook::HOOK_PROTOCOL_ID.to_string(),
        protocol_version: agent_semantic_hook::HOOK_PROTOCOL_VERSION.to_string(),
        project_root: ".".to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest,
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
            binary: manifest.binary.clone(),
            execution: manifest.execution,
            provider_command_prefix: Vec::new(),
            search_capabilities: manifest.search_capabilities.clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
            query_pack_descriptor: manifest.query_pack_descriptor.clone(),
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: manifest.source.default_source_roots.clone(),
                config_files: manifest.source.default_config_files.clone(),
                source_extensions: manifest.source.default_extensions.clone(),
                ignored_path_prefixes: manifest.source.default_ignored_path_prefixes.clone(),
            },
        }],
    };
    activation.providers[0].search_capabilities.owner_items =
        !activation.providers[0].search_capabilities.owner_items;
    let input = serde_json::to_string(&activation).expect("serialize drifted activation");

    let error = agent_semantic_hook::parse_activation(&input, &[manifest])
        .expect_err("search-capabilities drift must be rejected");
    let message = match error {
        agent_semantic_hook::AgentHookError::InvalidActivationConfig(message) => message,
        other => panic!("unexpected activation error variant: {other}"),
    };
    assert!(
        message.starts_with("provider activation does not match manifest identity"),
        "unexpected invalid-activation-config message: {message}"
    );
}

#[test]
fn activation_rejects_semantic_facts_descriptor_drift() {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.semantic_facts_descriptor.is_some())
        .expect("a builtin provider must own semantic-facts evidence");
    let manifest_digest =
        agent_semantic_hook::provider_manifest_digest(&manifest).expect("digest provider manifest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let mut activation = agent_semantic_hook::HookActivation {
        schema_id: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: agent_semantic_hook::HOOK_PROTOCOL_ID.to_string(),
        protocol_version: agent_semantic_hook::HOOK_PROTOCOL_VERSION.to_string(),
        project_root: ".".to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest,
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
            binary: manifest.binary.clone(),
            execution: manifest.execution,
            provider_command_prefix: Vec::new(),
            search_capabilities: manifest.search_capabilities.clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
            query_pack_descriptor: manifest.query_pack_descriptor.clone(),
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: manifest.source.default_source_roots.clone(),
                config_files: manifest.source.default_config_files.clone(),
                source_extensions: manifest.source.default_extensions.clone(),
                ignored_path_prefixes: manifest.source.default_ignored_path_prefixes.clone(),
            },
        }],
    };
    activation.providers[0]
        .semantic_facts_descriptor
        .as_mut()
        .expect("semantic-facts descriptor")
        .descriptor_id
        .push_str(".drift");
    let input = serde_json::to_string(&activation).expect("serialize drifted activation");

    let error = agent_semantic_hook::parse_activation(&input, &[manifest])
        .expect_err("semantic-facts descriptor drift must be rejected");
    let message = match error {
        agent_semantic_hook::AgentHookError::InvalidActivationConfig(message) => message,
        other => panic!("unexpected activation error variant: {other}"),
    };
    assert!(
        message.starts_with("provider activation does not match manifest identity"),
        "unexpected invalid-activation-config message: {message}"
    );
}
