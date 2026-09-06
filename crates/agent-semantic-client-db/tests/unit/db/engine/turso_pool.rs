// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{adaptive_turso_write_lane_ceiling, shared_turso_pool_entry};

fn unique_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-turso-pool-{label}-{}-{}.turso",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ))
}

#[tokio::test]
async fn entry_local_contention_cannot_block_an_unrelated_database_path() {
    let left = shared_turso_pool_entry(&unique_path("left")).await;
    let _left_writer = left.write_lanes.write().await;

    let started = tokio::time::Instant::now();
    let right = tokio::time::timeout(
        std::time::Duration::from_millis(10),
        shared_turso_pool_entry(&unique_path("right")),
    )
    .await
    .expect("an unrelated path must not wait on an entry-local lane lock");

    assert!(!std::sync::Arc::ptr_eq(&left, &right));
    assert!(started.elapsed() < std::time::Duration::from_millis(10));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_path_lookup_publishes_one_pool_entry() {
    let path = unique_path("same-path");
    let tasks = (0..64)
        .map(|_| {
            let path = path.clone();
            tokio::spawn(async move { shared_turso_pool_entry(&path).await })
        })
        .collect::<Vec<_>>();
    let mut entries = Vec::with_capacity(tasks.len());
    for task in tasks {
        entries.push(task.await.expect("pool lookup task"));
    }
    let first = &entries[0];
    assert!(
        entries
            .iter()
            .all(|entry| std::sync::Arc::ptr_eq(first, entry))
    );
}

#[test]
fn write_lane_ceiling_tracks_machine_parallelism() {
    let expected = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    assert_eq!(adaptive_turso_write_lane_ceiling(), expected);
}
