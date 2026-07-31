use agent_semantic_client_db::runtime_server::RuntimeServerEvent;
use agent_semantic_client_db::runtime_server_diagnostics::RuntimeServerDiagnostics;

#[tokio::test]
async fn latest_event_receipt_is_bounded_and_atomically_replaced() {
    let root = tempfile::tempdir().expect("create diagnostic fixture");
    let path = root.path().join("runtime-server-diagnostic.v1.json");
    let (sender, diagnostics) = RuntimeServerDiagnostics::start(path.clone())
        .await
        .expect("start diagnostics");
    sender
        .send(RuntimeServerEvent::ConnectionRejected("first".to_owned()))
        .expect("send first event");
    sender
        .send(RuntimeServerEvent::ConnectionTaskFailed(
            "second".to_owned(),
        ))
        .expect("send second event");
    drop(sender);
    diagnostics.join().await.expect("join diagnostics");

    let receipt: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.expect("read latest receipt"))
            .expect("decode latest receipt");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.runtime-server-diagnostic"
    );
    assert_eq!(receipt["event"]["kind"], "connection-task-failed");
    assert_eq!(receipt["event"]["detail"], "second");
    assert!(!path.with_extension("json.tmp").exists());
}
