// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::test_generation;

async fn await_task_scope_terminal(
    scope: &agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
) -> agent_semantic_workspace_scheduler::RuntimeServerTaskLifecycleReceipt {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let receipt = scope.receipt(0);
            if receipt.active == 0 {
                break receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Query materialization task must reach a lifecycle terminal")
}

#[tokio::test]
async fn query_materialization_task_completion_is_scope_accounted() {
    let generation = test_generation("query-task-completes");
    let task_scope = generation.task_scope.clone();
    let (completed, completion) = tokio::sync::oneshot::channel();

    generation
        .spawn_materialization("query-materialization-test", async move {
            let _ = completed.send(());
        })
        .expect("admit Query materialization task");
    completion.await.expect("Query task ran");

    let receipt = await_task_scope_terminal(&task_scope).await;
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.completed, 1);
    assert_eq!(receipt.cancelled, 0);
    assert_eq!(receipt.failed, 0);
}

#[tokio::test]
async fn dropping_a_generation_cancels_its_pending_query_tasks() {
    let generation = test_generation("query-task-cancelled-with-generation");
    let task_scope = generation.task_scope.clone();
    generation
        .spawn_materialization("query-materialization-test", std::future::pending())
        .expect("admit pending Query materialization task");
    tokio::task::yield_now().await;

    drop(generation);

    let receipt = await_task_scope_terminal(&task_scope).await;
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.completed, 0);
    assert_eq!(receipt.cancelled, 1);
    assert_eq!(receipt.failed, 0);
}
