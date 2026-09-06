// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::task::JoinSet;

use super::{durable_provider_binding_matches_current, spawn_runtime_owned_durability_task};

#[test]
fn provider_binding_drift_skips_restore_while_exact_binding_admits_it() {
    assert!(!durable_provider_binding_matches_current(
        None,
        Some("current")
    ));
    assert!(!durable_provider_binding_matches_current(
        Some("stale"),
        Some("current")
    ));
    assert!(durable_provider_binding_matches_current(
        Some("current"),
        Some("current")
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn cold_publication_does_not_wait_for_delayed_durability_attachment() {
    let tasks = Arc::new(Mutex::new(JoinSet::new()));
    let release = Arc::new(Notify::new());
    let completed = Arc::new(AtomicBool::new(false));

    tokio::time::timeout(
        Duration::from_millis(10),
        spawn_runtime_owned_durability_task(&tasks, {
            let release = Arc::clone(&release);
            let completed = Arc::clone(&completed);
            async move {
                release.notified().await;
                completed.store(true, Ordering::Release);
            }
        }),
    )
    .await
    .expect("Runtime must schedule durability without awaiting its completion");

    assert!(!completed.load(Ordering::Acquire));
    release.notify_one();

    let mut tasks = tasks.lock().await;
    tokio::time::timeout(Duration::from_millis(100), tasks.join_next())
        .await
        .expect("Runtime-owned durability task must remain joinable")
        .expect("scheduled durability task")
        .expect("durability task completion");
    assert!(completed.load(Ordering::Acquire));
}
