// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

#[tokio::test]
async fn diagnostic_journal_retains_the_latest_64_events() {
    let root = tempfile::tempdir().expect("create diagnostic journal fixture");
    let latest_path = root.path().join("runtime-server-diagnostic.v1.json");
    let journal_path = root
        .path()
        .join("runtime-server-diagnostic-journal.v1.json");
    let (sender, diagnostics) = RuntimeServerDiagnostics::start(latest_path)
        .await
        .expect("start diagnostics");
    for sequence in 1..=65 {
        sender
            .send(RuntimeServerEvent::ConnectionRejected(format!(
                "event-{sequence}"
            )))
            .expect("send journal event");
    }
    drop(sender);
    diagnostics.join().await.expect("join diagnostics");

    let receipt: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&journal_path)
            .await
            .expect("read diagnostic journal"),
    )
    .expect("decode diagnostic journal");
    let events = receipt["events"].as_array().expect("journal events array");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.runtime-server-diagnostic-journal"
    );
    assert_eq!(events.len(), 64);
    assert_eq!(events.first().expect("first event")["sequence"], 2);
    assert_eq!(events.last().expect("last event")["sequence"], 65);
    assert_eq!(
        events.first().expect("first event")["event"]["detail"],
        "event-2"
    );
    assert_eq!(
        events.last().expect("last event")["event"]["detail"],
        "event-65"
    );
    assert!(!journal_path.with_extension("json.tmp").exists());
}

#[tokio::test]
async fn diagnostic_journal_continues_across_runtime_restarts() {
    let root = tempfile::tempdir().expect("create restart diagnostic fixture");
    let latest_path = root.path().join("runtime-server-diagnostic.v1.json");
    for detail in ["before-restart-1", "before-restart-2"] {
        let (sender, diagnostics) = RuntimeServerDiagnostics::start(latest_path.clone())
            .await
            .expect("start diagnostics before restart");
        sender
            .send(RuntimeServerEvent::ConnectionRejected(detail.to_owned()))
            .expect("send diagnostic before restart");
        drop(sender);
        diagnostics.join().await.expect("join diagnostics");
    }

    let journal_path = root
        .path()
        .join("runtime-server-diagnostic-journal.v1.json");
    let receipt: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&journal_path)
            .await
            .expect("read restart diagnostic journal"),
    )
    .expect("decode restart diagnostic journal");
    let events = receipt["events"].as_array().expect("journal events array");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["sequence"], 1);
    assert_eq!(events[1]["sequence"], 2);
    assert_eq!(events[0]["event"]["detail"], "before-restart-1");
    assert_eq!(events[1]["event"]["detail"], "before-restart-2");
}

#[tokio::test]
async fn diagnostic_journal_accepts_more_than_its_bound_and_publishes_only_the_latest_event() {
    let root = tempfile::tempdir().expect("create bounded diagnostic fixture");
    let path = root.path().join("runtime-server-diagnostic.v1.json");
    let (sender, diagnostics) = RuntimeServerDiagnostics::start(path.clone())
        .await
        .expect("start bounded diagnostics");

    for sequence in 1..=65 {
        sender
            .send(RuntimeServerEvent::ConnectionRejected(format!(
                "event-{sequence}"
            )))
            .expect("send bounded diagnostic event");
    }
    drop(sender);
    diagnostics.join().await.expect("join bounded diagnostics");

    let receipt: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.expect("read latest receipt"))
            .expect("decode latest receipt");
    assert_eq!(receipt["sequence"], 65);
    assert_eq!(receipt["event"]["kind"], "connection-rejected");
    assert_eq!(receipt["event"]["detail"], "event-65");
    assert!(!path.with_extension("json.tmp").exists());
}

#[tokio::test]
async fn diagnostic_error_storm_is_bounded_and_drains_without_a_writer_leak() {
    let root = tempfile::tempdir().expect("create diagnostic pressure fixture");
    let path = root.path().join("runtime-server-diagnostic.v1.json");
    let journal_path = root
        .path()
        .join("runtime-server-diagnostic-journal.v1.json");
    let (publisher, diagnostics) = RuntimeServerDiagnostics::start(path)
        .await
        .expect("start bounded diagnostics");

    let mut rejected = 0_u64;
    for sequence in 0..100_000_u64 {
        if publisher
            .send(RuntimeServerEvent::ConnectionRejected(format!(
                "pressure-{sequence}"
            )))
            .is_err()
        {
            rejected += 1;
        }
    }
    assert!(
        rejected > 0,
        "a producer storm must meet bounded backpressure"
    );
    drop(publisher);
    tokio::time::timeout(std::time::Duration::from_secs(2), diagnostics.join())
        .await
        .expect("bounded diagnostics must drain within two seconds")
        .expect("bounded diagnostics must join");

    let receipt: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&journal_path)
            .await
            .expect("read bounded diagnostic journal"),
    )
    .expect("decode bounded diagnostic journal");
    assert_eq!(receipt["events"].as_array().expect("events").len(), 64);
}
