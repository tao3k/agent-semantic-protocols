use crate::runtime_server::generation_builder::{Stage, await_stage};
use std::time::{Duration, Instant};
use tokio::time::Instant as TokioInstant;

#[tokio::test]
async fn delayed_stage_uses_one_absolute_deadline() {
    let started = Instant::now();
    let deadline = TokioInstant::now() + Duration::from_millis(40);
    let first = await_stage(started, deadline, Stage::SourceBuilder, async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        Ok::<_, String>(())
    })
    .await;
    assert!(first.is_ok());
    let second = await_stage(started, deadline, Stage::SourceIndexCommit, async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        Ok::<_, String>(())
    })
    .await
    .expect_err("shared deadline must bound cumulative stage time");
    assert!(second.contains("stage=source-index-commit"));
    assert!(second.contains("elapsedMicros="));
}

#[tokio::test]
async fn pre_builder_scheduling_gap_produces_typed_bootstrap_timeout() {
    let started = Instant::now();
    let deadline = TokioInstant::now() + Duration::from_millis(60);
    tokio::time::sleep(Duration::from_millis(40)).await;

    let error = await_stage(started, deadline, Stage::WorkspaceBootstrap, async {
        std::future::pending::<Result<(), String>>().await
    })
    .await
    .expect_err("shared deadline must expire the real stage helper");

    assert!(error.contains("workspace-generation-build-stage-deadline-exceeded"));
    assert!(error.contains("stage=workspace-bootstrap"));
    assert!(error.contains("elapsedMicros="));
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "regression must remain bounded by the shared deadline"
    );
}
