// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::seqlock_json_memory::SeqlockJsonMemoryReader;
use agent_semantic_client_db::seqlock_json_memory::SeqlockJsonMemoryWriter;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct FixtureReceipt {
    sequence: u64,
    complement: u64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_publication_is_coherent_and_read_p99_is_sub_millisecond() {
    let directory = tempfile::tempdir().expect("temporary seqlock directory");
    let path = directory.path().join("fixture.v1.memory");
    let mut writer = SeqlockJsonMemoryWriter::create(&path, 64 * 1024)
        .await
        .expect("create seqlock writer");
    writer
        .publish(&FixtureReceipt {
            sequence: 0,
            complement: !0,
        })
        .expect("publish initial receipt");
    let reader = Arc::new(
        SeqlockJsonMemoryReader::open(&path)
            .await
            .expect("open seqlock reader"),
    );

    let writer_task = tokio::spawn(async move {
        for sequence in 1..=4_096_u64 {
            writer
                .publish(&FixtureReceipt {
                    sequence,
                    complement: !sequence,
                })
                .expect("publish concurrent receipt");
            if sequence % 32 == 0 {
                tokio::task::yield_now().await;
            }
        }
    });
    let mut readers = Vec::new();
    for _ in 0..8 {
        let reader = Arc::clone(&reader);
        readers.push(tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(1_024);
            for _ in 0..1_024 {
                let started = Instant::now();
                let (_, receipt): (_, FixtureReceipt) = loop {
                    match reader.read() {
                        Ok(receipt) => break receipt,
                        Err(error)
                            if error.contains("did not stabilize")
                                || error.contains("no committed generation") =>
                        {
                            tokio::task::yield_now().await;
                        }
                        Err(error) => panic!("seqlock read failed: {error}"),
                    }
                };
                latencies.push(started.elapsed());
                assert_eq!(receipt.complement, !receipt.sequence);
            }
            latencies
        }));
    }
    writer_task.await.expect("writer task");

    let mut latencies = Vec::with_capacity(8 * 1_024);
    for reader in readers {
        latencies.extend(reader.await.expect("reader task"));
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    assert!(
        p99 < Duration::from_millis(1),
        "seqlock read p99 must remain sub-millisecond: p99={p99:?}"
    );
}
