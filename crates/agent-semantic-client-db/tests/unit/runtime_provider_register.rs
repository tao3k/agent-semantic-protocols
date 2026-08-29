use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_provider_protocol::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_SCHEMA_VERSION,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResult,
    ProviderRegistrationDocument,
};
use serde_json::json;

fn identity_provider(language_id: &str, provider_id: &str) -> ProviderRegistrationDocument {
    ProviderRegistrationDocument {
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        registration: json!({
            "languageId": language_id,
            "providerId": provider_id,
        }),
    }
}

fn installed_capability(language_id: &str, provider_id: &str) -> ProviderRegistrationDocument {
    let mut provider = identity_provider(language_id, provider_id);
    provider.registration["namespace"] = json!(language_id);
    provider.registration["sourceInventory"] = json!({
        "packageRoots": [],
        "configFiles": [],
        "sourceExtensions": [format!(".{language_id}")],
        "projectResolution": {"entryMarkers": [format!("{language_id}.project")]},
        "documentResolution": null
    });
    provider.registration["searchCapabilities"] = json!({
        "ownerItems": true,
        "semanticFacts": true,
        "dependencyTopology": false,
        "dependencyTopologyMetadata": false
    });
    provider.registration["queryPackDescriptor"] = json!({});
    provider.registration["runtimeContract"] = json!({
        "transport": "http-json",
        "clientBinding": "schema-driven",
        "operations": [{
            "operation": "search.owner",
            "requestSchema": {
                "schemaId": "agent.semantic-protocols.search-owner-request",
                "schemaVersion": "1"
            },
            "responseSchema": {
                "schemaId": "agent.semantic-protocols.search-packet",
                "schemaVersion": "1"
            }
        }]
    });
    provider.registration["routes"] = json!([{
        "schemaId": "agent.semantic-protocols.provider-route",
        "schemaVersion": "1",
        "routeId": format!("{language_id}.search.owner"),
        "operation": "search.owner",
        "authority": "asp-server",
        "target": {"languageId": language_id, "providerId": provider_id},
        "inputs": [],
        "requirements": [{"kind": "state", "state": "provider-ready"}],
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
        "cache": {"authority": "asp-server", "scope": "workspace", "keySlots": []},
        "telemetry": {"spanName": "asp.route.search.owner", "attributeSlots": []}
    }]);
    provider
}

fn request(
    expected_generation: Option<u64>,
    request: ProviderRegisterOperation,
) -> ProviderRegisterRequest {
    ProviderRegisterRequest {
        schema_id: PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        expected_generation,
        request,
    }
}

#[tokio::test]
async fn external_provider_registration_publishes_one_immutable_generation() {
    let register = RuntimeProviderRegister::new();
    let response = register
        .apply(request(
            Some(0),
            ProviderRegisterOperation::Register {
                provider: installed_capability("zig", "asp-zig"),
            },
        ))
        .await
        .expect("register provider");
    response.validate().expect("valid response");
    let ProviderRegisterResult::Snapshot { snapshot } = response.result else {
        panic!("expected snapshot")
    };
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.providers[0].provider_id, "asp-zig");
}

#[tokio::test]
async fn stale_writer_receives_typed_generation_conflict() {
    let register = RuntimeProviderRegister::new();
    register
        .apply(request(
            Some(0),
            ProviderRegisterOperation::Register {
                provider: installed_capability("rust", "asp-rust"),
            },
        ))
        .await
        .expect("first writer");
    let response = register
        .apply(request(
            Some(0),
            ProviderRegisterOperation::Register {
                provider: installed_capability("python", "asp-python"),
            },
        ))
        .await
        .expect("stale writer response");
    assert_eq!(
        response.result,
        ProviderRegisterResult::GenerationConflict {
            actual_generation: 1
        }
    );
}

#[tokio::test]
async fn list_reads_the_resident_snapshot_without_writer_admission() {
    let register = RuntimeProviderRegister::from_seed(vec![identity_provider("rust", "asp-rust")])
        .expect("seed register");
    let response = register
        .apply(request(None, ProviderRegisterOperation::List))
        .await
        .expect("list providers");
    let ProviderRegisterResult::Snapshot { snapshot } = response.result else {
        panic!("expected snapshot")
    };
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.providers.len(), 1);
}

#[tokio::test]
async fn multiple_providers_can_implement_the_same_language() {
    let register = RuntimeProviderRegister::from_seed(vec![
        identity_provider("rust", "asp-rust"),
        identity_provider("rust", "asp-rust-experimental"),
    ])
    .expect("multiple implementations");
    assert_eq!(register.snapshot().providers.len(), 2);
}

#[tokio::test]
async fn external_provider_state_survives_runtime_server_reconstruction() {
    let temporary = tempfile::tempdir().expect("provider state directory");
    let store_path = temporary.path().join("provider-register-state.json");
    let register = RuntimeProviderRegister::from_seed_with_store(
        vec![identity_provider("rust", "asp-rust")],
        store_path.clone(),
    )
    .await
    .expect("load provider register");
    register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: installed_capability("zig", "asp-zig"),
            },
        ))
        .await
        .expect("persist external provider");
    drop(register);

    let restored = RuntimeProviderRegister::from_seed_with_store(
        vec![identity_provider("rust", "asp-rust")],
        store_path.clone(),
    )
    .await
    .expect("restore provider register");
    assert_eq!(restored.snapshot().providers.len(), 2);
    assert!(
        restored
            .snapshot()
            .providers
            .iter()
            .any(|provider| provider.provider_id == "asp-zig")
    );
}

#[tokio::test]
async fn builtin_identity_accepts_installed_routes_and_unregister_returns_to_seed() {
    let register = RuntimeProviderRegister::from_seed(vec![identity_provider("rust", "asp-rust")])
        .expect("seed register");
    let response = register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: installed_capability("rust", "asp-rust"),
            },
        ))
        .await
        .expect("publish installed provider routes");
    assert!(matches!(
        response.result,
        ProviderRegisterResult::Snapshot { .. }
    ));
    assert_eq!(register.snapshot().generation, 2);
    assert_eq!(register.compiled_routes("asp-rust").unwrap().len(), 1);

    register
        .apply(request(
            Some(2),
            ProviderRegisterOperation::Unregister {
                provider_id: "asp-rust".to_owned(),
            },
        ))
        .await
        .expect("return to seed identity");
    assert_eq!(register.snapshot().generation, 3);
    assert!(register.compiled_routes("asp-rust").is_none());
}

#[tokio::test]
async fn builtin_installed_capability_survives_runtime_owner_handoff() {
    let temporary = tempfile::tempdir().expect("provider state directory");
    let store_path = temporary.path().join("provider-register-state.json");
    let register = RuntimeProviderRegister::from_seed_with_store(
        vec![identity_provider("rust", "asp-rust")],
        store_path.clone(),
    )
    .await
    .expect("load provider capability register");
    register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: installed_capability("rust", "asp-rust"),
            },
        ))
        .await
        .expect("persist builtin provider capability");
    drop(register);

    let restored = RuntimeProviderRegister::from_seed_with_store(
        vec![identity_provider("rust", "asp-rust")],
        store_path.clone(),
    )
    .await
    .expect("restore provider capability register");
    assert!(restored.compiled_routes("asp-rust").is_some());
    assert_eq!(
        restored
            .installed_capability("rust")
            .expect("installed Rust capability")
            .provider_id,
        "asp-rust"
    );

    restored
        .apply(request(
            Some(restored.snapshot().generation),
            ProviderRegisterOperation::Unregister {
                provider_id: "asp-rust".to_owned(),
            },
        ))
        .await
        .expect("remove installed Rust capability");
    drop(restored);

    let identity_only = RuntimeProviderRegister::from_seed_with_store(
        vec![identity_provider("rust", "asp-rust")],
        store_path,
    )
    .await
    .expect("restore identity-only provider register");
    assert!(identity_only.compiled_routes("asp-rust").is_none());
}

#[tokio::test]
async fn warm_compiled_route_lookup_is_sub_millisecond() {
    let register = RuntimeProviderRegister::new();
    register
        .apply(request(
            Some(0),
            ProviderRegisterOperation::Register {
                provider: installed_capability("rust", "asp-rust"),
            },
        ))
        .await
        .expect("register installed provider capability");

    let started = std::time::Instant::now();
    for _ in 0..1_000 {
        assert!(register.compiled_routes("asp-rust").is_some());
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(1),
        "1,000 warm route lookups took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn operation_resolution_requires_an_installed_compiled_route() {
    let register = RuntimeProviderRegister::from_seed(vec![identity_provider("rust", "asp-rust")])
        .expect("seed register");
    let missing = register
        .resolve_route("rust", "search.owner")
        .expect_err("identity seed must not dispatch");
    assert!(
        missing.contains("operation-not-in-installed-capability"),
        "{missing}"
    );

    register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: installed_capability("rust", "asp-rust"),
            },
        ))
        .await
        .expect("register installed route");
    let (provider_id, route) = register
        .resolve_route("rust", "search.owner")
        .expect("resolve installed route");
    assert_eq!(provider_id, "asp-rust");
    assert_eq!(route.spec().route_id, "rust.search.owner");
}
