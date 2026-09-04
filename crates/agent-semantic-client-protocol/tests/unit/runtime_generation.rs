use super::RUNTIME_SERVER_GENERATION_MISMATCH;
use super::RuntimeServerGenerationIdentity;

fn identity(binary: &str, owner_epoch: u64) -> RuntimeServerGenerationIdentity {
    RuntimeServerGenerationIdentity::derive(
        binary,
        "agent.semantic-protocols.runtime-server-endpoint",
        "1",
        "transport",
        "catalog",
        owner_epoch,
    )
}

#[test]
fn new_client_cannot_bind_an_old_server_generation() {
    let error = identity("new", 2)
        .validate(&identity("old", 1))
        .unwrap_err();
    assert_eq!(error.reason_kind, RUNTIME_SERVER_GENERATION_MISMATCH);
}

#[test]
fn exact_generation_identity_is_admitted() {
    let expected = identity("same", 7);
    expected.validate(&expected).unwrap();
}
