use super::workspace_db_owner::{
    WorkspaceDbWriteOperation, WorkspaceDbWriteRequest, workspace_db_writer_channel,
};
use super::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderOwnerFingerprint,
    ProviderOwnerMetadata,
};

#[tokio::test(flavor = "current_thread")]
async fn concurrent_submissions_receive_one_deterministically_ordered_batch() {
    let (client, mut actor) = workspace_db_writer_channel(8);
    let first_client = client.clone();
    let second_client = client;
    let first = first_client.submit(request("request-a"));
    let second = second_client.submit(request("request-b"));
    let serve = async {
        tokio::task::yield_now().await;
        let batch = actor
            .receive_batch(8)
            .await
            .expect("receive workspace writer batch");
        let observed = batch
            .requests()
            .map(|(request, admission)| {
                (
                    request.request_id.clone(),
                    admission.sequence,
                    admission.batch_sequence,
                    admission.batch_index,
                )
            })
            .collect::<Vec<_>>();
        batch.complete(vec![
            Err("request-a-complete".to_owned()),
            Err("request-b-complete".to_owned()),
        ]);
        observed
    };

    let (first, second, observed) = tokio::join!(first, second, serve);

    assert_eq!(
        first.expect_err("first fixture response"),
        "request-a-complete"
    );
    assert_eq!(
        second.expect_err("second fixture response"),
        "request-b-complete"
    );
    assert_eq!(
        observed,
        vec![
            ("request-a".to_owned(), 0, 0, 0),
            ("request-b".to_owned(), 1, 0, 1),
        ]
    );
}

fn request(request_id: &str) -> WorkspaceDbWriteRequest {
    let scope = ProviderIncrementalScoped {
        project_root: "/workspace".to_owned(),
        workspace_identity: "workspace-1".to_owned(),
        provider_workspace_identity_digest: format!("{:064x}", 17),
        language_id: "rust".to_owned(),
        provider_id: "rust-provider".to_owned(),
        provider_workspace_root: "/workspace".to_owned(),
    };
    WorkspaceDbWriteRequest {
        workspace_identity: scope.workspace_identity.clone(),
        request_id: request_id.to_owned(),
        idempotency_key: format!("idempotency-{request_id}"),
        operation: WorkspaceDbWriteOperation::WriteProviderOwner(ProviderIncrementalOwnerWrite {
            scope,
            owner_path: "src/lib.rs".to_owned(),
            fingerprint: ProviderOwnerFingerprint {
                metadata: ProviderOwnerMetadata {
                    file_identity: "file-1".to_owned(),
                    size_bytes: 1,
                    modified_unix_nanos: 1,
                    change_time_unix_nanos: 1,
                },
                content_digest: format!("{:064x}", 23),
            },
            projection_completeness: "complete-owner".to_owned(),
            projections: Vec::new(),
        }),
    }
}
