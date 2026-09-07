// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use agent_semantic_http_json::HttpJsonResponse;
use agent_semantic_http_json::persistent::HttpJsonConnection;
use agent_semantic_http_json::serve_http_json_h2;
use tokio::net::TcpListener;
use tokio::sync::watch;

#[tokio::test(flavor = "multi_thread")]
async fn persistent_h2_uses_one_accept_for_256_multiplexed_frames() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, receiver) = watch::channel(false);
    let accepted = Arc::new(AtomicUsize::new(0));
    let accepted_for_handler = Arc::clone(&accepted);
    let server = tokio::spawn(async move {
        serve_http_json_h2(listener, receiver, move |_request| {
            accepted_for_handler.fetch_add(1, Ordering::Relaxed);
            async { HttpJsonResponse::json(200, &serde_json::json!({"ok": true})) }
        })
        .await
        .unwrap();
    });
    let client = Arc::new(HttpJsonConnection::connect(&endpoint).await.unwrap());
    let warm = client
        .post_json("/protocol/frame", b"{}".to_vec().into())
        .await
        .unwrap();
    assert_eq!(warm.0, 200);
    accepted.store(0, Ordering::Relaxed);

    let batch_started = Instant::now();
    let mut requests = Vec::new();
    for _ in 0..256 {
        let client = Arc::clone(&client);
        requests.push(tokio::spawn(async move {
            let started = Instant::now();
            client
                .post_json("/protocol/frame", b"{}".to_vec().into())
                .await
                .map(|response| (response, started.elapsed().as_micros() as u64))
        }));
    }
    let mut elapsed = Vec::with_capacity(256);
    for request in requests {
        let (response, micros) = request.await.unwrap().unwrap();
        assert_eq!(response.0, 200);
        elapsed.push(micros);
    }
    elapsed.sort_unstable();
    let p50 = elapsed[elapsed.len() / 2];
    let p99 = elapsed[(elapsed.len() * 99 / 100).min(elapsed.len() - 1)];
    let max = *elapsed.last().unwrap();
    let batch_micros = batch_started.elapsed().as_micros() as u64;
    let amortized_micros = batch_micros.div_ceil(elapsed.len() as u64);
    eprintln!(
        "persistent_h2 accept=1 batchMicros={batch_micros} amortizedMicros={amortized_micros} p50Micros={p50} p99Micros={p99} maxMicros={max}"
    );
    assert!(
        batch_micros < 50_000,
        "256-frame batch exceeded 50ms: {batch_micros}us"
    );
    assert!(
        amortized_micros <= 250,
        "amortized frame cost exceeded 250us: {amortized_micros}us"
    );
    assert_eq!(accepted.load(Ordering::Relaxed), 256);
    let client = Arc::try_unwrap(client).unwrap_or_else(|_| panic!("client still referenced"));
    client.close().await.unwrap();
    shutdown.send(true).unwrap();
    server.await.unwrap();
}
