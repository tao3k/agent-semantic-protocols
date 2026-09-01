use super::*;
use serde_json::json;
use tokio_stream::StreamExt;

fn encode(request: &serde_json::Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(request).map_err(|error| error.to_string())
}

fn fail_encode(_: &serde_json::Value) -> Result<Vec<u8>, String> {
    Err("injected-encode-failure".to_owned())
}

#[tokio::test]
async fn writer_stamps_sequence_in_actual_priority_wire_order() {
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(32);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);
    data_tx.send(json!({"requestId": "data-1"})).await.unwrap();
    control_tx
        .send(json!({"requestId": "cancel-data-1"}))
        .await
        .unwrap();
    let (terminal, _terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, 0, encode, terminal);

    let first: serde_json::Value = serde_json::from_slice(&stream.next().await.unwrap()).unwrap();
    let second: serde_json::Value = serde_json::from_slice(&stream.next().await.unwrap()).unwrap();
    assert_eq!(first["requestId"], "cancel-data-1");
    assert_eq!(first["sequence"], 1);
    assert_eq!(second["requestId"], "data-1");
    assert_eq!(second["sequence"], 2);
}

#[tokio::test]
async fn writer_serializes_thirty_two_concurrent_producers() {
    const CALLERS: usize = 32;
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(CALLERS);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel(1);
    let sends = (0..CALLERS).map(|caller| {
        let data_tx = data_tx.clone();
        tokio::spawn(async move {
            data_tx
                .send(json!({"requestId": format!("request-{caller}")}))
                .await
                .unwrap();
        })
    });
    for send in sends {
        send.await.unwrap();
    }
    let (terminal, _terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, 0, encode, terminal);

    for expected in 1..=CALLERS as u64 {
        let request: serde_json::Value =
            serde_json::from_slice(&stream.next().await.unwrap()).unwrap();
        assert_eq!(request["sequence"], expected);
    }
}

#[tokio::test]
async fn writer_terminalizes_sequence_overflow_once() {
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(1);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel(1);
    data_tx
        .send(json!({"requestId": "overflow"}))
        .await
        .unwrap();
    let (terminal, terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, u64::MAX, encode, terminal);

    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(
        terminal_rx.borrow().clone(),
        Some(PriorityJsonStreamTerminal::SequenceOverflow)
    );
}

#[tokio::test]
async fn writer_terminalizes_encode_failure_once() {
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(1);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel(1);
    data_tx.send(json!({"requestId": "encode"})).await.unwrap();
    let (terminal, terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, 0, fail_encode, terminal);

    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(
        terminal_rx.borrow().clone(),
        Some(PriorityJsonStreamTerminal::EncodeFailed(
            "injected-encode-failure".to_owned()
        ))
    );
}

#[tokio::test]
async fn writer_terminalizes_non_object_request_instead_of_panicking() {
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(1);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel(1);
    data_tx.send(json!(["not-an-object"])).await.unwrap();
    let (terminal, terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, 0, encode, terminal);

    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(
        terminal_rx.borrow().clone(),
        Some(PriorityJsonStreamTerminal::EncodeFailed(
            "priority JSON request must be an object".to_owned()
        ))
    );
}

#[tokio::test]
async fn writer_closes_cleanly_only_after_both_senders_close() {
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(1);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(1);
    let (terminal, terminal_rx) = tokio::sync::watch::channel(None);
    let mut stream = PriorityJsonStream::new(control_rx, data_rx, 0, encode, terminal);
    drop(data_tx);
    control_tx
        .send(json!({"requestId": "control-after-data-close"}))
        .await
        .unwrap();

    assert!(stream.next().await.is_some());
    drop(control_tx);
    assert!(stream.next().await.is_none());
    assert!(terminal_rx.borrow().is_none());
}
