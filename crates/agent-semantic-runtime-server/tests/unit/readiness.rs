use super::RuntimeServerReadinessListener;
use super::RuntimeServerReadinessPublisher;
use super::RuntimeServerReadinessReceipt;
use super::RuntimeServerReadinessState;

fn receipt(state: RuntimeServerReadinessState, token: &str) -> RuntimeServerReadinessReceipt {
    RuntimeServerReadinessReceipt {
        schema_id: RuntimeServerReadinessReceipt::SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        state,
        request_id: "request".to_owned(),
        readiness_token: token.to_owned(),
        process_id: 42,
        owner_epoch: 7,
        endpoint_binding_token: "binding".to_owned(),
        runtime_binary_identity: "binary".to_owned(),
        artifact_catalog_digest: "catalog".to_owned(),
        transport_contract_digest: "transport".to_owned(),
        reason_kind: Some("test".to_owned()),
        error: None,
    }
}

#[tokio::test]
async fn readiness_ready_roundtrip_and_cleanup() {
    let home = tempfile::Builder::new()
        .prefix("asp-rdy-")
        .tempdir_in("/tmp")
        .unwrap();
    let listener = RuntimeServerReadinessListener::bind(home.path(), "request", "token")
        .await
        .unwrap();
    let publisher =
        RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "token")
            .await
            .unwrap();
    publisher
        .publish(receipt(RuntimeServerReadinessState::Ready, "wrong"))
        .await
        .unwrap();
    let received = listener.receive(42).await.unwrap();
    assert_eq!(received.state, RuntimeServerReadinessState::Ready);
    let path = listener.socket_path().to_owned();
    listener.cleanup().await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn stale_token_and_malformed_payload_are_ignored() {
    let home = tempfile::Builder::new()
        .prefix("asp-rdy-")
        .tempdir_in("/tmp")
        .unwrap();
    let listener = RuntimeServerReadinessListener::bind(home.path(), "request", "token")
        .await
        .unwrap();
    let stale =
        RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "stale")
            .await
            .unwrap();
    let good = RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "token")
        .await
        .unwrap();
    stale
        .publish(receipt(RuntimeServerReadinessState::Ready, "stale"))
        .await
        .unwrap();
    good.publish(receipt(RuntimeServerReadinessState::Ready, "token"))
        .await
        .unwrap();
    assert_eq!(listener.receive(42).await.unwrap().readiness_token, "token");
    listener.cleanup().await.unwrap();
}
