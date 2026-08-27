pub async fn await_provider_runtime_ready_terminal<Receipt, Ready, Closed>(
    ready: Ready,
    cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    response_closed: Closed,
) -> Option<Result<Receipt, String>>
where
    Ready: std::future::Future<Output = Result<Receipt, String>>,
    Closed: std::future::Future<Output = ()>,
{
    tokio::select! {
        ready = ready => Some(ready),
        _ = cancellation.cancelled() => Some(Err(
            "runtime-generation-cancelled: provider readiness wait cancelled".to_owned()
        )),
        _ = response_closed => None,
    }
}
