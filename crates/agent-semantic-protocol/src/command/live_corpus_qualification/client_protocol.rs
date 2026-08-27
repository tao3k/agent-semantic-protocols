//! Live Corpus qualification through the ordinary typed ASP Client application API.

use std::path::Path;

use agent_semantic_client::{
    LanguageCommandClient, LanguageCommandOperation, LanguageCommandRequest,
};
use agent_semantic_client_protocol::{
    AspClientExactQueryFailure, AspClientExactQueryRequest, AspClientExactQueryResponse,
    AspClientSearchRequest, ClientFrame, ClientOutcome,
};
use agent_semantic_search_projection::RuntimeProviderSearchReceipt;

use super::contract::QualificationCase;

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PublicRouteFailure {
    reason_kind: String,
    message: String,
    #[serde(default)]
    details: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq)]
pub(super) enum PublicRouteTerminal {
    Queued(AspClientExactQueryFailure),
    Building(AspClientExactQueryFailure),
    Ready(serde_json::Value),
    Failed(PublicRouteFailure),
    Cancelled,
}

impl PublicRouteTerminal {
    fn require_ready(self, route: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::Ready(payload) => Ok(payload),
            Self::Queued(failure) => Err(format!(
                "Live Corpus public route remained Queued: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Building(failure) => Err(format!(
                "Live Corpus public route remained Building: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Failed(failure) => Err(format!(
                "Live Corpus public route failed: route={route} reasonKind={} message={}",
                failure.reason_kind, failure.message
            )),
            Self::Cancelled => Err(format!(
                "Live Corpus public route was cancelled: route={route}"
            )),
        }
    }
}

pub(super) fn typed_terminal(frame: ClientFrame) -> Result<PublicRouteTerminal, String> {
    match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } => Ok(PublicRouteTerminal::Ready(payload)),
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        } => Ok(PublicRouteTerminal::Cancelled),
        ClientFrame::Response {
            outcome: ClientOutcome::Error | ClientOutcome::StaleGeneration,
            error: Some(error),
            ..
        } => {
            let failure = serde_json::from_value::<PublicRouteFailure>(error)
                .map_err(|error| format!("decode Live Corpus typed route failure: {error}"))?;
            if let Some(details) = failure.details.as_ref()
                && let Ok(generation_failure) =
                    serde_json::from_value::<AspClientExactQueryFailure>(details.clone())
            {
                return Ok(match generation_failure.reason_kind.as_str() {
                    "runtime-generation-queued" => PublicRouteTerminal::Queued(generation_failure),
                    "runtime-generation-building" => {
                        PublicRouteTerminal::Building(generation_failure)
                    }
                    _ => PublicRouteTerminal::Failed(failure),
                });
            }
            Ok(PublicRouteTerminal::Failed(failure))
        }
        other => Err(format!(
            "Live Corpus public route returned an invalid terminal frame: {other:?}"
        )),
    }
}

async fn dispatch_ready<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    route: &str,
    operation: LanguageCommandOperation,
) -> Result<serde_json::Value, String> {
    let response = client
        .dispatch(LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new(language_id),
            operation,
            project_root: project_root.to_path_buf(),
            machine_readable: true,
        })
        .await?;
    typed_terminal(response.frame)?.require_ready(route)
}

#[derive(Debug)]
pub(super) struct PublicQualificationEvidence {
    pub(super) search: RuntimeProviderSearchReceipt,
    pub(super) source: AspClientExactQueryResponse,
    pub(super) callable_skeleton: AspClientExactQueryResponse,
    pub(super) zero_match: RuntimeProviderSearchReceipt,
}

pub(super) async fn qualify_public_client_case<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    case: &QualificationCase,
) -> Result<PublicQualificationEvidence, String> {
    let search = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        &case.search.method,
        &case.search.terms,
    )
    .await?;
    if search.candidate_count < case.search.minimum_candidates {
        return Err(format!(
            "Live Corpus public search returned too few candidates: case={} candidates={} minimum={}",
            case.case_id, search.candidate_count, case.search.minimum_candidates
        ));
    }
    if search.resident_read_elapsed_micros > case.search.maximum_resident_micros {
        return Err(format!(
            "Live Corpus public search exceeded resident budget: case={} elapsedMicros={} maximumMicros={}",
            case.case_id, search.resident_read_elapsed_micros, case.search.maximum_resident_micros
        ));
    }
    let selector = search.selectors.first().ok_or_else(|| {
        format!(
            "Live Corpus public search returned no parser-owned selector: case={}",
            case.case_id
        )
    })?;
    let source = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "source",
    )
    .await?;
    let callable_skeleton = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "callable-skeleton",
    )
    .await?;
    for (projection, response) in [
        ("source", &source),
        ("callable-skeleton", &callable_skeleton),
    ] {
        if response.resident_read_elapsed_micros > case.query.maximum_resident_micros {
            return Err(format!(
                "Live Corpus public query exceeded resident budget: case={} projection={projection} elapsedMicros={} maximumMicros={}",
                case.case_id,
                response.resident_read_elapsed_micros,
                case.query.maximum_resident_micros
            ));
        }
    }
    let observed_terminal_events = std::collections::BTreeSet::from([
        "runtime_resident_search_terminal",
        "runtime_exact_projection_terminal",
    ]);
    if let Some(missing) = case
        .required_telemetry_events
        .iter()
        .find(|event| !observed_terminal_events.contains(event.as_str()))
    {
        return Err(format!(
            "Live Corpus public route did not observe required typed terminal event: case={} event={missing}",
            case.case_id
        ));
    }
    let zero_match = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        "lexical",
        &case.zero_match_terms,
    )
    .await?;
    if zero_match.candidate_count != 0 {
        return Err(format!(
            "Live Corpus public zero-match search returned candidates: case={} candidates={}",
            case.case_id, zero_match.candidate_count
        ));
    }
    Ok(PublicQualificationEvidence {
        search,
        source,
        callable_skeleton,
        zero_match,
    })
}

async fn search_receipt<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    operation: &str,
    terms: &[String],
) -> Result<RuntimeProviderSearchReceipt, String> {
    let payload = dispatch_ready(
        client,
        project_root,
        language_id,
        "search",
        LanguageCommandOperation::Search(AspClientSearchRequest {
            schema_id: "agent.semantic-protocols.asp-client-search-request".to_owned(),
            schema_version: "1".to_owned(),
            operation: operation.to_owned(),
            query: terms.join(" "),
        }),
    )
    .await?;
    let receipt = serde_json::from_value::<RuntimeProviderSearchReceipt>(payload)
        .map_err(|error| format!("decode Live Corpus public search payload: {error}"))?;
    receipt.validate()?;
    if receipt.language_id != language_id {
        return Err(format!(
            "Live Corpus public search language drift: expected={language_id} actual={}",
            receipt.language_id
        ));
    }
    Ok(receipt)
}

async fn public_query<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    selector: &str,
    projection: &str,
) -> Result<AspClientExactQueryResponse, String> {
    let payload = dispatch_ready(
        client,
        project_root,
        language_id,
        "query",
        LanguageCommandOperation::ExactQuery(AspClientExactQueryRequest {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
            schema_version: "1".to_owned(),
            selector: selector.to_owned(),
            projection: projection.to_owned(),
        }),
    )
    .await?;
    let response = serde_json::from_value::<AspClientExactQueryResponse>(payload)
        .map_err(|error| format!("decode Live Corpus public {projection} payload: {error}"))?;
    response.validate()?;
    if response.language_id != language_id {
        return Err(format!(
            "Live Corpus public query language drift: expected={language_id} actual={}",
            response.language_id
        ));
    }
    Ok(response)
}
