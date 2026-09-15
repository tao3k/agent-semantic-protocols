// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Scheme-lowered Query Playbook qualification over the public Runtime route.

use std::path::Path;
use std::time::Instant;

use agent_semantic_client_protocol::{AspClientWorkspaceQueryPlaybookRequest, ClientFrame};

use crate::{LanguageCommandClient, LanguageCommandOperation, LanguageCommandRequest};

use super::client_protocol::typed_terminal;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkspaceQueryQualificationReceipt {
    pub(crate) operation_id: String,
    pub(crate) language_id: String,
    pub(crate) provider_id: String,
    pub(crate) generation_digest: String,
    pub(crate) root_digest: String,
    pub(crate) selector: String,
    pub(crate) projection: String,
    pub(crate) request_profile: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) result: serde_json::Value,
    pub(crate) elapsed_micros: u64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceQueryPlaybookReceipt {
    schema_id: String,
    schema_version: String,
    protocol_id: String,
    protocol_version: String,
    request_id: String,
    project_workspace_identity: String,
    worktree_instance_id: String,
    runtime_execution_binding: serde_json::Value,
    runtime_workspace_execution_publication_digest: String,
    runtime_bundle_digest: String,
    source_generation_digest: String,
    source_root_digest: String,
    request_profile: String,
    request_plane_elapsed_micros: u64,
    projection: String,
    requested_selectors: Vec<String>,
    materializations: Vec<WorkspaceQueryMaterialization>,
    terminal: WorkspaceQueryTerminal,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceQueryMaterialization {
    selector: String,
    language_id: String,
    provider_id: String,
    owner_path: String,
    projection: String,
    source_content_digest: String,
    bytes: Vec<u8>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceQueryTerminal {
    state: String,
    terminal_count: u64,
    #[serde(default)]
    reason_kind: Option<String>,
}

pub(crate) async fn public_query<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    selector: &str,
    projection: &str,
    scheme_template: &str,
) -> Result<WorkspaceQueryQualificationReceipt, String> {
    let request =
        workspace_query_qualification_request(language_id, selector, projection, scheme_template)?;
    let round_trip_started = Instant::now();
    let response = client
        .dispatch(LanguageCommandRequest {
            language_id: crate::LanguageId::new(language_id),
            operation: LanguageCommandOperation::WorkspaceQueryPlaybook(request),
            project_root: project_root.to_path_buf(),
            machine_readable: true,
        })
        .await?;
    let operation_id = match &response.frame {
        ClientFrame::Response { request_id, .. } => request_id.as_str().to_owned(),
        frame => {
            return Err(format!(
                "Live Corpus Query Playbook returned a non-response frame: {frame:?}"
            ));
        }
    };
    let payload = typed_terminal(response.frame)?.require_ready("workspace.query.playbook")?;
    let receipt = serde_json::from_value::<WorkspaceQueryPlaybookReceipt>(payload)
        .map_err(|error| format!("decode Live Corpus public {projection} QueryBook: {error}"))?;
    let round_trip_elapsed_micros = round_trip_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    validate_receipt_identity(&receipt, &operation_id)?;
    validate_complete_terminal(&receipt, selector, projection)?;
    validate_content_identity(&receipt)?;
    let materialization = receipt
        .materializations
        .into_iter()
        .next()
        .expect("length checked by complete terminal admission");
    validate_materialization(&materialization, language_id, selector, projection)?;
    let result = decode_projection_result(projection, &materialization.bytes)?;
    if round_trip_elapsed_micros < receipt.request_plane_elapsed_micros {
        return Err("Live Corpus Query Playbook Runtime time exceeds client round trip".to_owned());
    }
    Ok(WorkspaceQueryQualificationReceipt {
        operation_id,
        language_id: materialization.language_id,
        provider_id: materialization.provider_id,
        generation_digest: receipt.source_generation_digest,
        root_digest: receipt.source_root_digest,
        selector: materialization.selector,
        projection: materialization.projection,
        request_profile: receipt.request_profile,
        bytes: materialization.bytes,
        result,
        elapsed_micros: receipt.request_plane_elapsed_micros,
    })
}

fn validate_receipt_identity(
    receipt: &WorkspaceQueryPlaybookReceipt,
    operation_id: &str,
) -> Result<(), String> {
    if receipt.schema_id != "agent.semantic-protocols.query-playbook-materialization-receipt"
        || receipt.schema_version != "1"
        || receipt.protocol_id != "agent.semantic-protocols.query-playbook"
        || receipt.protocol_version != "1"
        || receipt.request_id != operation_id
        || receipt.project_workspace_identity.is_empty()
        || receipt.worktree_instance_id.is_empty()
        || !receipt.runtime_execution_binding.is_object()
        || receipt
            .runtime_workspace_execution_publication_digest
            .is_empty()
        || receipt.runtime_bundle_digest.is_empty()
    {
        return Err(format!(
            "Live Corpus Query Playbook receipt identity is invalid: schemaId={} schemaVersion={} protocolId={} protocolVersion={} requestId={} expectedRequestId={} projectWorkspaceIdentityEmpty={} worktreeInstanceIdEmpty={} runtimeExecutionBindingObject={} executionPublicationDigestEmpty={} runtimeBundleDigestEmpty={}",
            receipt.schema_id,
            receipt.schema_version,
            receipt.protocol_id,
            receipt.protocol_version,
            receipt.request_id,
            operation_id,
            receipt.project_workspace_identity.is_empty(),
            receipt.worktree_instance_id.is_empty(),
            receipt.runtime_execution_binding.is_object(),
            receipt
                .runtime_workspace_execution_publication_digest
                .is_empty(),
            receipt.runtime_bundle_digest.is_empty(),
        ));
    }
    Ok(())
}

fn validate_complete_terminal(
    receipt: &WorkspaceQueryPlaybookReceipt,
    selector: &str,
    projection: &str,
) -> Result<(), String> {
    if receipt.terminal.state != "ready"
        || receipt.terminal.terminal_count != 1
        || receipt.terminal.reason_kind.is_some()
        || receipt.projection != projection
        || receipt.requested_selectors != [selector]
        || receipt.materializations.len() != 1
        || !matches!(
            receipt.request_profile.as_str(),
            "materialized" | "resident-hit"
        )
    {
        return Err(format!(
            "Live Corpus Query Playbook did not return one complete {projection} materialization: state={} terminalCount={} reasonKind={:?} receiptProjection={} requestedSelectors={:?} materializationCount={} requestProfile={}",
            receipt.terminal.state,
            receipt.terminal.terminal_count,
            receipt.terminal.reason_kind,
            receipt.projection,
            receipt.requested_selectors,
            receipt.materializations.len(),
            receipt.request_profile,
        ));
    }
    Ok(())
}

fn validate_materialization(
    materialization: &WorkspaceQueryMaterialization,
    language_id: &str,
    selector: &str,
    projection: &str,
) -> Result<(), String> {
    let expected_owner =
        agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
            selector,
        )?
        .owner_path()?;
    if materialization.selector != selector
        || materialization.language_id != language_id
        || materialization.provider_id.is_empty()
        || materialization.owner_path != expected_owner
        || materialization.projection != projection
        || materialization.bytes.is_empty()
        || materialization.source_content_digest
            != blake3::hash(&materialization.bytes).to_hex().to_string()
    {
        return Err(format!(
            "Live Corpus Query Playbook materialization authority drift: projection={projection}"
        ));
    }
    Ok(())
}

fn validate_content_identity(receipt: &WorkspaceQueryPlaybookReceipt) -> Result<(), String> {
    if !receipt.source_generation_digest.starts_with("blake3-256:")
        || receipt.source_generation_digest.len() != 75
        || receipt.source_root_digest.len() != 64
    {
        return Err("Live Corpus Query Playbook content identity is invalid".to_owned());
    }
    Ok(())
}

fn decode_projection_result(projection: &str, bytes: &[u8]) -> Result<serde_json::Value, String> {
    if projection == "callable-skeleton" {
        serde_json::from_slice(bytes).map_err(|error| {
            format!("decode Live Corpus callable-skeleton materialization: {error}")
        })
    } else {
        Ok(serde_json::Value::Array(
            bytes.iter().copied().map(serde_json::Value::from).collect(),
        ))
    }
}

pub(crate) fn workspace_query_qualification_request(
    producer_id: &str,
    selector: &str,
    projection: &str,
    scheme_template: &str,
) -> Result<AspClientWorkspaceQueryPlaybookRequest, String> {
    let profile = include_str!("../../../../../schemas/language-schema-profiles.json");
    let profile: serde_json::Value = serde_json::from_str(profile)
        .map_err(|error| format!("decode embedded Query producer profile registry: {error}"))?;
    let producer = profile
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|profile| {
            profile
                .get("languageId")
                .and_then(serde_json::Value::as_str)
                == Some(producer_id)
        })
        .ok_or_else(|| format!("Query producer profile is not registered: {producer_id}"))?;
    let axes = producer
        .get("searchProducerAxes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("Query producer profile has no registered axis: {producer_id}"))?;
    let language_axis = axes.iter().any(|axis| axis.as_str() == Some("language"));
    let document_axis = axes.iter().any(|axis| axis.as_str() == Some("document"));
    if language_axis == document_axis {
        return Err(format!(
            "Live Corpus Query producer must resolve to exactly one axis: {producer_id}"
        ));
    }
    let source = render_workspace_query_scheme_source(scheme_template, selector)?;
    let parsed = agent_semantic_search::parse_progressive_query_args(&[
        "query".to_owned(),
        "playbook".to_owned(),
        source,
    ])
    .map_err(|error| format!("lower Live Corpus Query Scheme: {error}"))?;
    let agent_semantic_search::ProgressiveQueryRequest::Selector {
        language,
        documents,
        selectors,
        projection: parsed_projection,
        ..
    } = parsed;
    if language.as_deref() != language_axis.then_some(producer_id)
        || documents.as_deref() != document_axis.then_some(producer_id)
        || selectors != [selector]
        || parsed_projection != projection
    {
        return Err(format!(
            "Live Corpus Query Scheme identity drift: producer={producer_id} selector={selector} projection={projection}"
        ));
    }
    Ok(AspClientWorkspaceQueryPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language,
        documents,
        selectors,
        projection: parsed_projection,
    })
}

pub(crate) fn render_workspace_query_scheme_source(
    scheme_template: &str,
    selector: &str,
) -> Result<String, String> {
    if scheme_template.matches("{{selector}}").count() != 1 {
        return Err(
            "Live Corpus Query Scheme template must contain exactly one {{selector}} slot"
                .to_owned(),
        );
    }
    let selector = serde_json::to_string(selector)
        .map_err(|error| format!("encode Query Scheme selector: {error}"))?;
    Ok(scheme_template.replace("{{selector}}", &selector))
}

pub(crate) fn source_query_scheme_template(producer_id: &str) -> Result<String, String> {
    let profile = include_str!("../../../../../schemas/language-schema-profiles.json");
    let profile: serde_json::Value = serde_json::from_str(profile)
        .map_err(|error| format!("decode embedded Query producer profile registry: {error}"))?;
    let producer = profile
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|profile| {
            profile
                .get("languageId")
                .and_then(serde_json::Value::as_str)
                == Some(producer_id)
        })
        .ok_or_else(|| format!("Query producer profile is not registered: {producer_id}"))?;
    let axes = producer
        .get("searchProducerAxes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("Query producer profile has no registered axis: {producer_id}"))?;
    let language = axes.iter().any(|axis| axis.as_str() == Some("language"));
    let documents = axes.iter().any(|axis| axis.as_str() == Some("document"));
    let axis = match (language, documents) {
        (true, false) => "language",
        (false, true) => "documents",
        _ => {
            return Err(format!(
                "Query producer must resolve to exactly one axis: {producer_id}"
            ));
        }
    };
    Ok(format!(
        "(query (producers ({axis} {producer_id})) (select (selectors {{{{selector}}}}) (projection source) (output json)))"
    ))
}
