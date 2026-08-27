//! Runtime Server-owned Hook health projection.

pub(super) async fn run_doctor(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: asp hook doctor".to_owned());
    }
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let health = agent_semantic_client_db::runtime_server_health::
        cached_runtime_server_health_for_state_home(&state_home).await?;
    println!(
        "[hook-doctor] status={} authority=runtime-server state={:?} elapsedMicros={}",
        if health.is_healthy() {
            "ok"
        } else {
            "degraded"
        },
        health.resident.state,
        health.elapsed_micros,
    );
    Ok(())
}
