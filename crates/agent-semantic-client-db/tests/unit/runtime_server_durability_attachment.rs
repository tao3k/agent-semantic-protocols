// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::task::JoinSet;

use super::{durable_runtime_bundle_matches_current, spawn_runtime_owned_durability_task};

#[test]
fn runtime_bundle_drift_skips_restore_while_exact_binding_admits_it() {
    assert!(!durable_runtime_bundle_matches_current(
        None,
        Some("current")
    ));
    assert!(!durable_runtime_bundle_matches_current(
        Some("stale"),
        Some("current")
    ));
    assert!(durable_runtime_bundle_matches_current(
        Some("current"),
        Some("current")
    ));
}

#[test]
fn successor_build_lane_is_transferred_to_the_durability_attachment() {
    let source = include_str!("../../src/runtime_server/core.rs");
    let acquire = source
        .find("let generation_durability_guard = durability_lane.lock_owned().await")
        .expect("generation build must acquire its workspace durability lane");
    let publication = source[acquire..]
        .find("generation_publication.publish(query_publication)")
        .map(|offset| acquire + offset)
        .expect("resident query-generation publication");
    let transfer = source[publication..]
        .find("let _generation_durability_guard = generation_durability_guard")
        .map(|offset| publication + offset)
        .expect("durability task must own the lane after resident publication");
    let commit = source[transfer..]
        .find(".commit_source_index_generation(")
        .map(|offset| transfer + offset)
        .expect("durable Source Index commit");
    assert!(
        acquire < publication && publication < transfer && transfer < commit,
        "one same-workspace lane must cover build through durability without delaying resident publication"
    );
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
