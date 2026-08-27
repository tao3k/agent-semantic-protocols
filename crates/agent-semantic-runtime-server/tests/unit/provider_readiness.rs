use agent_semantic_runtime_server::provider_readiness::await_provider_runtime_ready_terminal;

#[tokio::test]
async fn pending_provider_readiness_terminalizes_when_generation_is_cancelled() {
    let cancellation =
        agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn(await_provider_runtime_ready_terminal(
        {
            let entered = std::sync::Arc::clone(&entered);
            async move {
                entered.notify_one();
                std::future::pending::<
                    Result<
                        agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
                        String,
                    >,
                >()
                .await
            }
        },
        cancellation.clone(),
        std::future::pending::<()>(),
    ));

    entered.notified().await;
    cancellation.cancel();
    let error = task
        .await
        .expect("readiness task")
        .expect("cancelled readiness emits a terminal")
        .expect_err("cancelled readiness is not Ready");
    assert!(error.contains("runtime-generation-cancelled"));
}
