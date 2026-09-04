use agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID;
use agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION;
use agent_semantic_provider_protocol::ProviderRegisterOperation;
use agent_semantic_provider_protocol::ProviderRegisterRequest;
use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use agent_semantic_provider_protocol::builtin_provider_registrations;
use serde_json::json;

fn provider(language_id: &str, provider_id: &str) -> ProviderRegistrationDocument {
    ProviderRegistrationDocument {
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        registration: json!({
            "languageId": language_id,
            "providerId": provider_id,
            "namespace": language_id,
            "sourceInventory": {
                "packageRoots": [],
                "configFiles": [],
                "sourceExtensions": [format!(".{language_id}")],
                "projectResolution": {
                    "entryMarkers": [format!("{language_id}.project")]
                },
                "documentResolution": null
            },
            "searchCapabilities": {
                "ownerItems": true,
                "semanticFacts": true,
                "dependencyTopology": false,
                "dependencyTopologyMetadata": false
            },
            "queryPackDescriptor": {},
            "routes": [{
                "schemaId": "agent.semantic-protocols.provider-route",
                "schemaVersion": "1",
                "routeId": format!("{language_id}.search"),
                "operation": "search",
                "authority": "asp-server",
                "target": {
                    "languageId": language_id,
                    "providerId": provider_id
                },
                "requestSchema": {
                    "schemaId": "agent.semantic-protocols.runtime-provider-search-request",
                    "schemaVersion": "1"
                },
                "inputs": [],
                "requirements": [],
                "effects": {
                    "access": "read",
                    "idempotent": true,
                    "cancellable": true,
                    "concurrency": "shared-read",
                    "streaming": false
                },
                "output": {
                    "schema": {
                        "schemaId": "agent.semantic-protocols.search-packet",
                        "schemaVersion": "1"
                    },
                    "mediaType": "application/json"
                },
                "failureSchemaIds": ["agent.semantic-protocols.route-failure"],
                "cache": {
                    "authority": "asp-server",
                    "scope": "workspace",
                    "keySlots": []
                },
                "telemetry": {
                    "spanName": "asp.route.search",
                    "attributeSlots": []
                }
            }]
        }),
    }
}

#[test]
fn external_provider_uses_the_same_registration_document_contract() {
    let request = ProviderRegisterRequest {
        schema_id: PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        expected_generation: Some(7),
        request: ProviderRegisterOperation::Register {
            provider: provider("external-test", "asp-external-test"),
        },
    };
    request.validate().expect("external provider registration");
    let encoded = serde_json::to_value(&request).expect("serialize request");
    let decoded: ProviderRegisterRequest =
        serde_json::from_value(encoded).expect("deserialize request");
    assert_eq!(decoded, request);
}

#[test]
fn initialize_rejects_duplicate_provider_identity() {
    let request = ProviderRegisterRequest {
        schema_id: PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        expected_generation: None,
        request: ProviderRegisterOperation::Initialize {
            providers: vec![provider("rust", "asp-rust"), provider("rust-2", "asp-rust")],
        },
    };
    assert_eq!(
        request.validate(),
        Err("duplicate providerId `asp-rust`".to_owned())
    );
}

#[test]
fn registration_identity_must_match_the_schema_document() {
    let mut registration = provider("python", "asp-python");
    registration.registration["providerId"] = json!("asp-rust");
    assert_eq!(
        registration.validate(),
        Err("provider registration providerId `asp-rust` does not match `asp-python`".to_owned())
    );
}

#[test]
fn installed_capability_requires_source_inventory_owned_by_the_client_server() {
    let mut registration = provider("rust", "asp-rust");
    registration
        .registration
        .as_object_mut()
        .expect("registration object")
        .remove("sourceInventory");
    assert_eq!(
        registration.compiled_routes(),
        Err("installed provider capability must declare sourceInventory".to_owned())
    );
}

#[test]
fn builtin_providers_are_loaded_from_the_schema_register() {
    let providers = builtin_provider_registrations().expect("builtin provider register");
    assert!(
        providers
            .iter()
            .any(|provider| provider.provider_id == "asp-rust")
    );
    assert!(
        providers
            .iter()
            .any(|provider| provider.provider_id == "asp-python")
    );
    assert_eq!(providers.len(), 5);
    assert!(
        providers
            .iter()
            .all(|provider| !matches!(provider.language_id.as_str(), "org" | "md"))
    );
}
