// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceRequest;
use agent_semantic_client_db::runtime_search_service::runtime_search_service_channel;
use tokio_stream::StreamExt;

#[tokio::test]
async fn provider_readiness_wait_carries_generation_cancellation_to_the_service_actor() {
    let (service, mut requests) = runtime_search_service_channel();
    let cancellation = GenerationCancellation::new();
    let request_cancellation = cancellation.clone();
    let request = tokio::spawn(async move {
        service
            .provider_runtime_await_ready(
                std::path::PathBuf::from("/workspace"),
                "rust".to_owned(),
                request_cancellation,
            )
            .await
    });

    let RuntimeSearchServiceRequest::ProviderRuntimeAwaitReady {
        cancellation: actor_cancellation,
        response,
        ..
    } = requests.next().await.expect("provider readiness request")
    else {
        panic!("expected provider readiness request");
    };
    let actor = tokio::spawn(async move {
        actor_cancellation.cancelled().await;
        let _ = response.send(Err(
            "runtime-generation-cancelled: provider readiness wait cancelled".to_owned(),
        ));
    });

    cancellation.cancel();
    actor.await.expect("service actor terminal");
    let error = request
        .await
        .expect("request task")
        .expect_err("cancelled readiness is terminal");
    assert!(error.contains("runtime-generation-cancelled"));
}

#[tokio::test]
async fn provider_operation_carries_generation_cancellation_to_the_service_actor() {
    let (service, mut requests) = runtime_search_service_channel();
    let cancellation = GenerationCancellation::new();
    let request_cancellation = cancellation.clone();
    let request = tokio::spawn(async move {
        service
            .provider_operation(
                std::path::PathBuf::from("/workspace"),
                "rust".to_owned(),
                "project-resolution".to_owned(),
                Vec::new(),
                request_cancellation,
            )
            .await
    });

    let RuntimeSearchServiceRequest::ProviderOperation {
        cancellation: actor_cancellation,
        response,
        ..
    } = requests.next().await.expect("provider operation request")
    else {
        panic!("expected provider operation request");
    };
    let actor = tokio::spawn(async move {
        actor_cancellation.cancelled().await;
        let _ = response.send(Err(
            "runtime-generation-cancelled: provider operation cancelled".to_owned(),
        ));
    });

    cancellation.cancel();
    actor.await.expect("service actor terminal");
    let error = request
        .await
        .expect("request task")
        .expect_err("cancelled operation is terminal");
    assert!(error.contains("runtime-generation-cancelled"));
}
