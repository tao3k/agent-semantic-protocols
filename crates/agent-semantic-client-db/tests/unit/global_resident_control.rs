use agent_semantic_client_db::global_resident_control::{
    GlobalResidentControlReceipt, GlobalResidentOperation, GlobalResidentState,
    call_global_resident, prepare_global_resident_endpoint, read_global_resident_request,
    write_global_resident_receipt,
};

#[tokio::test(flavor = "current_thread")]
async fn global_endpoint_omits_workspace_and_process_identity() {
    let endpoint = prepare_global_resident_endpoint(
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        21,
        "binding-21",
    )
    .await
    .expect("prepare global resident endpoint");
    let value = serde_json::to_value(&endpoint).expect("encode global resident endpoint");
    let object = value.as_object().expect("endpoint JSON object");

    assert!(!object.contains_key("workspaceIdentity"));
    assert!(!object.contains_key("ownerPid"));
    assert!(endpoint.socket_path.len() <= 103);
}

#[tokio::test(flavor = "current_thread")]
async fn typed_global_status_and_restart_roundtrip() {
    let endpoint = prepare_global_resident_endpoint(
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        22,
        "binding-22",
    )
    .await
    .expect("prepare global resident endpoint");
    let listener =
        tokio::net::UnixListener::bind(&endpoint.socket_path).expect("bind global resident");
    let server_endpoint = endpoint.clone();
    let server = tokio::spawn(async move {
        for expected in [
            GlobalResidentOperation::Status,
            GlobalResidentOperation::Restart,
        ] {
            let (mut stream, _) = listener.accept().await.expect("accept global control");
            let request = read_global_resident_request(&mut stream)
                .await
                .expect("read global control request");
            assert_eq!(request.operation, expected);
            assert_eq!(
                request.transport_contract_digest,
                server_endpoint.transport_contract_digest
            );
            assert_eq!(request.owner_epoch, server_endpoint.owner_epoch);
            assert_eq!(request.binding_token, server_endpoint.binding_token);
            write_global_resident_receipt(
                &mut stream,
                &GlobalResidentControlReceipt {
                    schema_id: "agent.semantic-protocols.global-resident-control-receipt.v1"
                        .to_owned(),
                    schema_version: "1".to_owned(),
                    request_id: request.request_id,
                    state: if expected == GlobalResidentOperation::Restart {
                        GlobalResidentState::Draining
                    } else {
                        GlobalResidentState::Healthy
                    },
                    runtime_artifact_digest: server_endpoint.runtime_artifact_digest.clone(),
                    transport_contract_digest: server_endpoint.transport_contract_digest.clone(),
                    workspace_entry_count: 3,
                    reason: None,
                },
            )
            .await
            .expect("write global control receipt");
        }
    });

    let started = std::time::Instant::now();
    let status = call_global_resident(
        &endpoint,
        GlobalResidentOperation::Status,
        "status-1".to_owned(),
    )
    .await
    .expect("call global resident status");
    assert_eq!(status.state, GlobalResidentState::Healthy);
    assert_eq!(status.workspace_entry_count, 3);
    let restart = call_global_resident(
        &endpoint,
        GlobalResidentOperation::Restart,
        "restart-1".to_owned(),
    )
    .await
    .expect("call global resident restart");
    assert_eq!(restart.state, GlobalResidentState::Draining);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(10),
        "two warm global control roundtrips exceeded 10ms: {:?}",
        started.elapsed()
    );

    server.await.expect("join global resident server");
    std::fs::remove_file(&endpoint.socket_path).expect("remove global resident socket");
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_global_control_requests_do_not_serialize_on_one_connection() {
    const REQUEST_COUNT: usize = 64;
    let endpoint = prepare_global_resident_endpoint(
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        23,
        "binding-23",
    )
    .await
    .expect("prepare concurrent global resident endpoint");
    let listener =
        tokio::net::UnixListener::bind(&endpoint.socket_path).expect("bind global resident");
    let server_endpoint = endpoint.clone();
    let server = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        for _ in 0..REQUEST_COUNT {
            let (mut stream, _) = listener.accept().await.expect("accept global control");
            let endpoint = server_endpoint.clone();
            connections.spawn(async move {
                let request = read_global_resident_request(&mut stream)
                    .await
                    .expect("read concurrent control request");
                request
                    .validate_for_endpoint(&endpoint)
                    .expect("validate concurrent control request");
                write_global_resident_receipt(
                    &mut stream,
                    &GlobalResidentControlReceipt::healthy(request.request_id, &endpoint, 7),
                )
                .await
                .expect("write concurrent control receipt");
            });
        }
        while let Some(completed) = connections.join_next().await {
            completed.expect("join concurrent server connection");
        }
    });

    let started = std::time::Instant::now();
    let mut clients = tokio::task::JoinSet::new();
    for index in 0..REQUEST_COUNT {
        let endpoint = endpoint.clone();
        clients.spawn(async move {
            call_global_resident(
                &endpoint,
                GlobalResidentOperation::Status,
                format!("concurrent-{index}"),
            )
            .await
            .expect("call concurrent global resident")
        });
    }
    while let Some(completed) = clients.join_next().await {
        let receipt = completed.expect("join concurrent client");
        assert_eq!(receipt.state, GlobalResidentState::Healthy);
        assert_eq!(receipt.workspace_entry_count, 7);
    }
    let elapsed = started.elapsed();
    let average = elapsed / u32::try_from(REQUEST_COUNT).expect("request count fits u32");
    assert!(
        elapsed < std::time::Duration::from_millis(25),
        "{REQUEST_COUNT} concurrent warm control requests exceeded 25ms: {elapsed:?}"
    );
    assert!(
        average < std::time::Duration::from_millis(1),
        "average concurrent warm control request is not sub-millisecond: {average:?}"
    );

    server
        .await
        .expect("join concurrent global resident server");
    std::fs::remove_file(&endpoint.socket_path).expect("remove global resident socket");
}
