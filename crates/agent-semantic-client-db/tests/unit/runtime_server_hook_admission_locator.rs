use super::{RuntimeHookAdmissionLocatorAuthority, connect_hook_workspace_session_with_receipt};
use crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry;
use crate::runtime_server_control::{
    RuntimeServerEndpoint, runtime_server_transport_contract_digest,
};
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;

fn endpoint(root: &std::path::Path) -> RuntimeServerEndpoint {
    RuntimeServerEndpoint {
        binary_content_digest:
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        runtime_generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest: runtime_server_transport_contract_digest(),
        owner_epoch: 7,
        owner_process_id: 0,
        runtime_artifact_path: root.join("runtime/bin/asp").display().to_string(),
        runtime_binary_identity: RuntimeBinaryIdentity::Content {
            value: format!("blake3-256:{}", "2".repeat(64)),
            algorithm: "blake3-256".to_owned(),
        },
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::Content {
            value: format!("blake3-256:{}", "2".repeat(64)),
            algorithm: "blake3-256".to_owned(),
        },
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "3".repeat(64)),
        binding_token: "binding-token".to_owned(),
        socket_path: root
            .join("runtime/server/control.sock")
            .display()
            .to_string(),
        data_plane_socket_path: root.join("runtime/server/data.sock").display().to_string(),
        provider_plane_socket_path: root
            .join("runtime/server/providers.sock")
            .display()
            .to_string(),
        client_http_endpoint: "http://127.0.0.1:1".to_owned(),
        workspace_store_path: root.join("runtime/server/workspaces").display().to_string(),
        status_memory_path: root
            .join("runtime/server/status.memory")
            .display()
            .to_string(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reused_locator_lookup_p99_is_sub_millisecond_under_workspace_pressure() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut entries = std::collections::BTreeSet::new();
    let mut roots = Vec::new();
    for index in 0..8 {
        let project_root = root.path().join(format!("checkout-{index}"));
        tokio::fs::create_dir_all(&project_root)
            .await
            .expect("project root");
        entries.insert(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: format!("workspace-{index}"),
            project_root: project_root.clone(),
        });
        roots.push(project_root);
    }
    let mut authority =
        RuntimeHookAdmissionLocatorAuthority::start(root.path(), &endpoint(root.path()))
            .await
            .expect("locator authority");
    authority.publish(&entries).expect("publish");
    let cold_started = std::time::Instant::now();
    let (cold_session, cold_receipt) =
        connect_hook_workspace_session_with_receipt(root.path(), &roots[0])
            .await
            .expect("cold locator mapping");
    let cold = cold_started.elapsed();
    assert_eq!(cold_session.workspace_identity(), "workspace-0");
    assert_eq!(cold_receipt.git_discoveries, 0);
    assert_eq!(cold_receipt.package_scope_resolutions, 0);
    assert_eq!(cold_receipt.canonicalize_calls, 0);
    assert_eq!(cold_receipt.endpoint_json_reads, 0);
    assert_eq!(cold_receipt.locator_memory_opens, 1);
    eprintln!(
        "coldStages locatorOpenNanos={} documentReadNanos={} sessionConstructNanos={}",
        cold_receipt.locator_open_nanos,
        cold_receipt.document_read_nanos,
        cold_receipt.session_construct_nanos
    );
    assert!(
        cold < std::time::Duration::from_millis(1),
        "cold typed locator lookup must remain below 1ms: cold={cold:?}"
    );
    eprintln!(
        "{{\"schemaId\":\"agent.semantic-protocols.runtime-hook-admission-performance-receipt.v1\",\"schemaVersion\":\"1\",\"mode\":\"cold\",\"elapsedNanos\":{},\"gitDiscoveries\":0,\"packageScopeResolutions\":0,\"canonicalizeCalls\":0,\"endpointJsonReads\":0,\"locatorMemoryOpens\":1}}",
        cold.as_nanos()
    );

    let mut tasks = Vec::new();
    for sample in 0..1024 {
        let state_home = root.path().to_path_buf();
        let lookup_root = roots[sample % roots.len()].clone();
        tasks.push(tokio::spawn(async move {
            let started = std::time::Instant::now();
            let (session, receipt) =
                connect_hook_workspace_session_with_receipt(&state_home, &lookup_root)
                    .await
                    .expect("pressure lookup");
            assert!(session.workspace_identity().starts_with("workspace-"));
            assert_eq!(receipt.git_discoveries, 0);
            assert_eq!(receipt.package_scope_resolutions, 0);
            assert_eq!(receipt.canonicalize_calls, 0);
            assert_eq!(receipt.endpoint_json_reads, 0);
            assert_eq!(receipt.locator_memory_opens, 0);
            started.elapsed()
        }));
    }
    let mut samples = Vec::with_capacity(tasks.len());
    for task in tasks {
        samples.push(task.await.expect("join"));
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99 / 100).min(samples.len() - 1)];
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "reused typed locator lookup p99 must remain below 1ms: p99={p99:?}"
    );
    eprintln!(
        "{{\"schemaId\":\"agent.semantic-protocols.runtime-hook-admission-performance-receipt.v1\",\"schemaVersion\":\"1\",\"mode\":\"reused-pressure\",\"workspaces\":8,\"sessions\":1024,\"p99Nanos\":{},\"gitDiscoveries\":0,\"packageScopeResolutions\":0,\"canonicalizeCalls\":0,\"endpointJsonReads\":0,\"locatorMemoryOpens\":0}}",
        p99.as_nanos()
    );
}
