use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_provider_protocol::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_SCHEMA_VERSION,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResult,
    ProviderRegistrationDocument,
};
use serde_json::json;

fn provider(language_id: &str, provider_id: &str) -> ProviderRegistrationDocument {
    ProviderRegistrationDocument {
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        registration: json!({
            "languageId": language_id,
            "providerId": provider_id,
        }),
    }
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
                provider: provider("zig", "asp-zig"),
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
                provider: provider("rust", "asp-rust"),
            },
        ))
        .await
        .expect("first writer");
    let response = register
        .apply(request(
            Some(0),
            ProviderRegisterOperation::Register {
                provider: provider("python", "asp-python"),
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
    let register = RuntimeProviderRegister::from_seed(vec![provider("rust", "asp-rust")])
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
        provider("rust", "asp-rust"),
        provider("rust", "asp-rust-experimental"),
    ])
    .expect("multiple implementations");
    assert_eq!(register.snapshot().providers.len(), 2);
}

#[tokio::test]
async fn external_provider_state_survives_runtime_server_reconstruction() {
    let temporary = tempfile::tempdir().expect("provider state directory");
    let store_path = temporary.path().join("provider-register-state.json");
    let register = RuntimeProviderRegister::from_seed_with_store(
        vec![provider("rust", "asp-rust")],
        store_path.clone(),
    )
    .await
    .expect("load provider register");
    register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: provider("zig", "asp-zig"),
            },
        ))
        .await
        .expect("persist external provider");
    drop(register);

    let restored = RuntimeProviderRegister::from_seed_with_store(
        vec![provider("rust", "asp-rust")],
        store_path,
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
async fn runtime_mutation_cannot_override_source_registered_builtin_provider() {
    let register = RuntimeProviderRegister::from_seed(vec![provider("rust", "asp-rust")])
        .expect("seed register");
    let response = register
        .apply(request(
            Some(1),
            ProviderRegisterOperation::Register {
                provider: provider("rust", "asp-rust"),
            },
        ))
        .await
        .expect("typed rejection");
    assert!(matches!(
        response.result,
        ProviderRegisterResult::Rejected { ref reason_kind, .. }
            if reason_kind == "builtin-provider-owned"
    ));
    assert_eq!(register.snapshot().generation, 1);
}
