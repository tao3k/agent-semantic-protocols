// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::runtime_server::generation_builder::{Stage, await_stage};
use crate::runtime_server_admission::WorkspaceGenerationBuildFailure;
use std::time::{Duration, Instant};
use tokio::time::Instant as TokioInstant;

#[tokio::test]
async fn delayed_stage_uses_one_absolute_deadline() {
    let deadline = TokioInstant::now() + Duration::from_millis(40);
    let first = await_stage(
        "workspace-test",
        "source-builder",
        Stage::SourceBuilder,
        async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            Ok::<_, WorkspaceGenerationBuildFailure>(())
        },
    )
    .await;
    assert!(first.is_ok());
    let second = await_stage(
        "workspace-test",
        "source-index-commit",
        Stage::SourceIndexCommit,
        async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok::<_, WorkspaceGenerationBuildFailure>(())
        },
    )
    .await
    .expect_err("shared deadline must bound cumulative stage time");
    assert_eq!(
        second.stage,
        crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceIndexCommit
    );
    assert!(second.message.contains("elapsedMicros="));
}

#[tokio::test]
async fn pre_builder_scheduling_gap_produces_typed_bootstrap_timeout() {
    let started = Instant::now();
    let deadline = TokioInstant::now() + Duration::from_millis(60);
    tokio::time::sleep(Duration::from_millis(40)).await;

    let error = await_stage(
        "workspace-test",
        "workspace-bootstrap",
        Stage::WorkspaceBootstrap,
        async { std::future::pending::<Result<(), WorkspaceGenerationBuildFailure>>().await },
    )
    .await
    .expect_err("shared deadline must expire the real stage helper");

    assert_eq!(
        error.stage,
        crate::runtime_server_admission::WorkspaceGenerationFailureStage::WorkspaceBootstrap
    );
    assert!(error.message.contains("elapsedMicros="));
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "regression must remain bounded by the shared deadline"
    );
}
