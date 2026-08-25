//! Typed ASP Client lifecycle qualification over the Runtime-published HTTP endpoint.

use std::time::Instant;

use agent_semantic_client_protocol::{ClientFrame, ClientOutcome, ClientWorkspaceIdentity};
use agent_semantic_client_server::AspClientProtocolHttpClient;

use super::contract::QualificationCase;

fn first_selector(value: &serde_json::Value) -> Option<&str> {
    if let Some(selector) = value.get("selector").and_then(serde_json::Value::as_str) {
        return Some(selector);
    }
    match value {
        serde_json::Value::Array(values) => values.iter().find_map(first_selector),
        serde_json::Value::Object(values) => values.values().find_map(first_selector),
        _ => None,
    }
}

fn ready_result(frame: &ClientFrame, phase: &str) -> Result<serde_json::Value, String> {
    match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => Ok(result.clone()),
        ClientFrame::Response { outcome, error, .. } => Err(format!(
            "public ASP Client {phase} returned outcome={outcome:?} error={error:?}"
        )),
        _ => Err(format!(
            "public ASP Client {phase} returned a non-response frame"
        )),
    }
}

pub(super) async fn qualify_client_protocol_case(
    endpoint: &str,
    workspace_identity: &str,
    project_root: &std::path::Path,
    case: &QualificationCase,
) -> Result<(), String> {
    let workspace_identity = ClientWorkspaceIdentity::new(workspace_identity)?;
    let mut client = AspClientProtocolHttpClient::connect(
        endpoint,
        workspace_identity,
        project_root.display().to_string(),
    )
    .await?;

    let search_started = Instant::now();
    eprintln!(
        "[live-corpus-phase] state=started phase=public-search case={}",
        case.case_id
    );
    let search_frame = client
        .dispatch(
            &format!("{}.search", case.language_id),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-search-request",
                "schemaVersion": "1",
                "operation": case.search.method,
                "view": case.search.view,
                "query": case.search.terms.join(" "),
                "terms": case.search.terms,
            }),
        )
        .await?;
    let search_result = ready_result(&search_frame, "search")?;
    let selector = first_selector(&search_result)
        .ok_or_else(|| {
            format!(
                "public ASP Client search returned no selector: case={}",
                case.case_id
            )
        })?
        .to_owned();
    eprintln!(
        "[live-corpus-phase] state=completed phase=public-search case={} elapsedMicros={}",
        case.case_id,
        search_started.elapsed().as_micros()
    );

    for projection in ["source", "callable-skeleton"] {
        let query_started = Instant::now();
        eprintln!(
            "[live-corpus-phase] state=started phase=public-query case={} projection={projection}",
            case.case_id
        );
        let query_frame = client
            .dispatch(
                &format!("{}.query", case.language_id),
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
                    "schemaVersion": "1",
                    "selector": selector,
                    "projection": projection,
                }),
            )
            .await?;
        let _ = ready_result(&query_frame, projection)?;
        eprintln!(
            "[live-corpus-phase] state=completed phase=public-query case={} projection={} elapsedMicros={}",
            case.case_id,
            projection,
            query_started.elapsed().as_micros()
        );
    }

    let catalog = client.initialize().await?;
    let method = catalog
        .methods
        .iter()
        .find(|method| {
            method.cancellable
                && method.method == agent_semantic_client_protocol::CANCELLATION_PROBE_METHOD
        })
        .ok_or_else(|| "client catalog has no cancellable lifecycle probe".to_owned())?;
    let request_id = client.next_request_id()?;
    let started = Instant::now();
    let request_future = async {
        eprintln!(
            "[live-corpus-phase] state=started phase=client-request case={} requestId={}",
            case.case_id,
            request_id.as_str()
        );
        let result = client
            .request_with_id(
                request_id.clone(),
                method.method.as_str(),
                serde_json::json!({}),
            )
            .await;
        let (outcome, reason_kind, message) = match &result {
            Ok(ClientFrame::Response { outcome, error, .. }) => (
                format!("{outcome:?}"),
                error
                    .as_ref()
                    .and_then(|error| error.get("reasonKind"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("none"),
                error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("none")
                    .replace(char::is_whitespace, "-"),
            ),
            Ok(_) => ("non-response".to_owned(), "none", "none".to_owned()),
            Err(error) => (
                "transport-error".to_owned(),
                "transport-error",
                error.replace(char::is_whitespace, "-"),
            ),
        };
        eprintln!(
            "[live-corpus-phase] state={} phase=client-request case={} requestId={} outcome={} reasonKind={} message={} elapsedMicros={}",
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            },
            case.case_id,
            request_id.as_str(),
            outcome,
            reason_kind,
            message,
            started.elapsed().as_micros()
        );
        result
    };
    let cancel_future = async {
        eprintln!(
            "[live-corpus-phase] state=started phase=client-cancel case={} requestId={}",
            case.case_id,
            request_id.as_str()
        );
        let result = client.cancel(request_id.clone()).await;
        eprintln!(
            "[live-corpus-phase] state={} phase=client-cancel case={} requestId={} elapsedMicros={}",
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            },
            case.case_id,
            request_id.as_str(),
            started.elapsed().as_micros()
        );
        result
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
