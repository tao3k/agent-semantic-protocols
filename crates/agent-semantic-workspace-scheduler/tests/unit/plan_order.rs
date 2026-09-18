#[tokio::test]
async fn starts_every_task_and_restores_plan_order() {
    let start_barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let mut tasks = tokio::task::JoinSet::new();
    for (plan_index, label, completion_delay_millis) in [(0, "first", 20), (1, "second", 0)] {
        let start_barrier = start_barrier.clone();
        tasks.spawn(async move {
            start_barrier.wait().await;
            tokio::time::sleep(std::time::Duration::from_millis(completion_delay_millis)).await;
            Ok::<_, String>((plan_index, label))
        });
    }
    tokio::time::timeout(std::time::Duration::from_secs(1), start_barrier.wait())
        .await
        .expect("every workspace task must start without a leaf concurrency cap");

    let completed = super::join_tasks_in_plan_order(tasks, 2)
        .await
        .expect("join every workspace task");
    assert_eq!(completed, [(0, "first"), (1, "second")]);
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
