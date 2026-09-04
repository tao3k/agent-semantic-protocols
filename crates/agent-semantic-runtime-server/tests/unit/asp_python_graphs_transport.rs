use super::decode_receipt;
use super::terminalize_pending;
use super::wire_terminal_message;

#[test]
fn receipt_requires_json_object() {
    assert!(decode_receipt(br#"{"requestId":"request-1"}"#).is_ok());
    assert!(decode_receipt(br#"[]"#).is_err());
}

#[test]
fn generic_wire_failures_map_to_stable_graph_protocol_terminals() {
    assert_eq!(
        wire_terminal_message(
            agent_semantic_provider_transport::PriorityJsonStreamTerminal::SequenceOverflow,
        ),
        "state=failed reasonKind=asp-python-graphs-wire-sequence-overflow"
    );
    assert_eq!(
        wire_terminal_message(
            agent_semantic_provider_transport::PriorityJsonStreamTerminal::EncodeFailed(
                "injected".to_owned(),
            ),
        ),
        "state=failed reasonKind=asp-python-graphs-wire-encode-failed error=injected"
    );
}

#[tokio::test]
async fn typed_wire_terminal_drains_pending_exactly_once() {
    let terminal = tokio::sync::Mutex::new(None);
    let (first_tx, first_rx) = tokio::sync::oneshot::channel();
    let (second_tx, second_rx) = tokio::sync::oneshot::channel();
    let pending = tokio::sync::Mutex::new(std::collections::HashMap::from([
        ("first".to_owned(), first_tx),
        ("second".to_owned(), second_tx),
    ]));
    let message = "state=failed reasonKind=asp-python-graphs-wire-closed".to_owned();

    terminalize_pending(&terminal, &pending, message.clone()).await;
    terminalize_pending(&terminal, &pending, "second-terminal".to_owned()).await;

    assert_eq!(terminal.lock().await.as_deref(), Some(message.as_str()));
    assert_eq!(first_rx.await.unwrap().unwrap_err(), message);
    assert_eq!(second_rx.await.unwrap().unwrap_err(), message);
    assert!(pending.lock().await.is_empty());
}
