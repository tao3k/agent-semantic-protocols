//! Typed ASP Client lifecycle qualification over the Runtime-published HTTP endpoint.

use std::time::Instant;

use agent_semantic_client_protocol::{ClientFrame, ClientOutcome, ClientWorkspaceIdentity};
use agent_semantic_client_server::AspClientProtocolHttpClient;

use super::contract::QualificationCase;

pub(super) async fn qualify_client_protocol_case(
    endpoint: &str,
    workspace_identity: &str,
    project_root: &std::path::Path,
    workspace_generation: &str,
    case: &QualificationCase,
) -> Result<(), String> {
    let workspace_identity = ClientWorkspaceIdentity::new(workspace_identity)?;
    let mut client = AspClientProtocolHttpClient::connect(
        endpoint,
        workspace_identity,
        project_root.display().to_string(),
    )
    .await?;
    let catalog = client.initialize().await?;
    let method = catalog
        .methods
        .iter()
        .find(|method| method.cancellable && method.method.ends_with(".search"))
        .or_else(|| catalog.methods.iter().find(|method| method.cancellable))
        .ok_or_else(|| "client catalog has no cancellable search method".to_owned())?;
    let request_id = client.next_request_id()?;
    let started = Instant::now();
    let request_future = client.request_with_id(
        request_id.clone(),
        method.method.as_str(),
        workspace_generation,
        serde_json::json!({"terms": case.search.terms, "view": case.search.view}),
    );
    let cancel_future = async {
        tokio::task::yield_now().await;
        client.cancel(request_id).await
    };
    let (request_result, cancel_result) = tokio::join!(request_future, cancel_future);
    let cancel_frame = cancel_result?;
    let request_frame = request_result?;
    if !matches!(
        cancel_frame,
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        }
    ) || !matches!(
        request_frame,
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        }
    ) {
        return Err(format!(
            "client cancellation did not produce typed Cancelled outcomes: case={} elapsedMicros={}",
            case.case_id,
            started.elapsed().as_micros()
        ));
    }
    let _ = client.shutdown().await?;
    Ok(())
}
