// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-bound admission for one selector-native Query Playbook request.

use std::fmt;

use agent_semantic_content_identity::ProjectWorkspaceBinding;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use serde_json::Map;
use serde_json::Value;

pub const QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.query-playbook-materialization-request";
pub const QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_VERSION: &str = "1";
pub const QUERY_PLAYBOOK_MATERIALIZATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.query-playbook-materialization-receipt";
pub const QUERY_PLAYBOOK_MATERIALIZATION_RECEIPT_SCHEMA_VERSION: &str = "1";

/// One immutable Query Playbook request admitted against the current Runtime binding.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryPlaybookMaterializationRequest {
    packet: Value,
}

impl QueryPlaybookMaterializationRequest {
    /// Admit canonical selectors independently of any prior Search result.
    pub fn admit_for_runtime(
        packet: Value,
        expected_runtime_binding: &RuntimeExecutionBinding,
        expected_execution_publication_digest: &str,
        expected_runtime_bundle_digest: &str,
        manifest_project_workspace: &ProjectWorkspaceBinding,
    ) -> Result<Self, QueryPlaybookMaterializationError> {
        admit_expected_runtime_binding(expected_runtime_binding, manifest_project_workspace)?;

        let packet_object = object(&packet, "packet")?;
        if ["topologyLibraryDigest", "topologyClosureDigest"]
            .iter()
            .any(|field| packet_object.contains_key(*field))
        {
            return invalid(
                "schema-invalid",
                "Query request must not carry Search topology identity",
            );
        }
        text_eq(
            packet_object,
            "schemaId",
            QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_ID,
        )?;
        text_eq(
            packet_object,
            "schemaVersion",
            QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_VERSION,
        )?;
        text_eq(
            packet_object,
            "protocolId",
            "agent.semantic-protocols.query-playbook",
        )?;
        text_eq(packet_object, "protocolVersion", "1")?;
        text(packet_object, "requestId")?;
        match text(packet_object, "projection")? {
            "source" | "callable-skeleton" => {}
            _ => return invalid("schema-invalid", "unsupported Query projection"),
        }

        let actual_runtime_binding: RuntimeExecutionBinding = serde_json::from_value(
            packet_object
                .get("runtimeExecutionBinding")
                .cloned()
                .ok_or_else(|| error("schema-invalid", "runtimeExecutionBinding is required"))?,
        )
        .map_err(|decode_error| {
            error(
                "schema-invalid",
                format!("runtimeExecutionBinding is invalid: {decode_error}"),
            )
        })?;
        expected_runtime_binding
            .admits(&actual_runtime_binding)
            .map_err(|binding_error| {
                error(
                    "query-playbook-runtime-binding-mismatch",
                    format!("Runtime execution binding drifted: {binding_error:?}"),
                )
            })?;
        if text(packet_object, "runtimeWorkspaceExecutionPublicationDigest")?
            != expected_execution_publication_digest
            || text(packet_object, "runtimeBundleDigest")? != expected_runtime_bundle_digest
        {
            return invalid(
                "query-playbook-execution-publication-mismatch",
                "Query execution publication or outer Runtime bundle identity drifted",
            );
        }
        if text(packet_object, "projectWorkspaceIdentity")?
            != expected_runtime_binding
                .project_workspace
                .project_workspace_identity()
            || text(packet_object, "worktreeInstanceId")?
                != expected_runtime_binding.worktree_instance_id
        {
            return invalid(
                "query-playbook-runtime-context-mismatch",
                "Query project/worktree context differs from the admitted Runtime binding",
            );
        }

        let selectors = array(packet_object, "selectors")?;
        if selectors.is_empty()
            || selectors.iter().any(|selector| {
                selector
                    .as_str()
                    .is_none_or(|value| !value.contains("://") || !value.contains("#item/"))
            })
            || selectors
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != selectors.len()
        {
            return invalid(
                "schema-invalid",
                "Query selectors must be unique canonical selectors in request order",
            );
        }

        Ok(Self { packet })
    }

    pub fn as_json(&self) -> &Value {
        &self.packet
    }
}

/// One all-or-nothing terminal for an admitted Query Playbook request.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryPlaybookMaterializationReceipt {
    packet: Value,
}

impl QueryPlaybookMaterializationReceipt {
    pub fn admit_for_runtime(
        packet: Value,
        request: &QueryPlaybookMaterializationRequest,
        expected_runtime_binding: &RuntimeExecutionBinding,
        expected_execution_publication_digest: &str,
        expected_runtime_bundle_digest: &str,
        manifest_project_workspace: &ProjectWorkspaceBinding,
    ) -> Result<Self, QueryPlaybookMaterializationError> {
        admit_expected_runtime_binding(expected_runtime_binding, manifest_project_workspace)?;

        let receipt = object(&packet, "packet")?;
        if ["topologyLibraryDigest", "topologyClosureDigest"]
            .iter()
            .any(|field| receipt.contains_key(*field))
        {
            return invalid(
                "schema-invalid",
                "Query receipt must not carry Search topology identity",
            );
        }
        text_eq(
            receipt,
            "schemaId",
            QUERY_PLAYBOOK_MATERIALIZATION_RECEIPT_SCHEMA_ID,
        )?;
        text_eq(
            receipt,
            "schemaVersion",
            QUERY_PLAYBOOK_MATERIALIZATION_RECEIPT_SCHEMA_VERSION,
        )?;
        text_eq(
            receipt,
            "protocolId",
            "agent.semantic-protocols.query-playbook",
        )?;
        text_eq(receipt, "protocolVersion", "1")?;

        let actual_runtime_binding: RuntimeExecutionBinding = serde_json::from_value(
            receipt
                .get("runtimeExecutionBinding")
                .cloned()
                .ok_or_else(|| error("schema-invalid", "runtimeExecutionBinding is required"))?,
        )
        .map_err(|decode_error| {
            error(
                "schema-invalid",
                format!("runtimeExecutionBinding is invalid: {decode_error}"),
            )
        })?;
        expected_runtime_binding
            .admits(&actual_runtime_binding)
            .map_err(|binding_error| {
                error(
                    "query-playbook-runtime-binding-mismatch",
                    format!("Runtime execution binding drifted: {binding_error:?}"),
                )
            })?;
        if text(receipt, "runtimeWorkspaceExecutionPublicationDigest")?
            != expected_execution_publication_digest
            || text(receipt, "runtimeBundleDigest")? != expected_runtime_bundle_digest
        {
            return invalid(
                "query-playbook-execution-publication-mismatch",
                "Query receipt execution publication or outer Runtime bundle identity drifted",
            );
        }
        let admitted_request = object(request.as_json(), "request")?;
        for field in [
            "requestId",
            "projectWorkspaceIdentity",
            "worktreeInstanceId",
            "runtimeWorkspaceExecutionPublicationDigest",
            "runtimeBundleDigest",
            "projection",
        ] {
            if receipt.get(field) != admitted_request.get(field) {
                return invalid(
                    "query-playbook-request-binding-mismatch",
                    format!("receipt {field} differs from the admitted request"),
                );
            }
        }
        if receipt.get("requestedSelectors") != admitted_request.get("selectors") {
            return invalid(
                "query-playbook-request-binding-mismatch",
                "receipt selector set differs from the admitted request",
            );
        }

        let terminal = receipt
            .get("terminal")
            .ok_or_else(|| error("schema-invalid", "terminal is required"))
            .and_then(|value| object(value, "terminal"))?;
        if terminal.get("terminalCount").and_then(Value::as_u64) != Some(1) {
            return invalid(
                "query-playbook-terminal-count-mismatch",
                "Query Playbook must emit exactly one terminal",
            );
        }
        let state = text(terminal, "state")?;
        let materializations = array(receipt, "materializations")?;
        match state {
            "ready" => validate_complete_materializations(receipt, admitted_request)?,
            "failed" => {
                text(terminal, "reasonKind")?;
                if !materializations.is_empty() {
                    return invalid(
                        "query-playbook-failure-exposed-partial-materialization",
                        "failed Query Playbook terminal must expose zero materializations",
                    );
                }
            }
            _ => return invalid("schema-invalid", "unsupported Query terminal state"),
        }

        Ok(Self { packet })
    }

    pub fn as_json(&self) -> &Value {
        &self.packet
    }
}

fn admit_expected_runtime_binding(
    expected_runtime_binding: &RuntimeExecutionBinding,
    manifest_project_workspace: &ProjectWorkspaceBinding,
) -> Result<(), QueryPlaybookMaterializationError> {
    expected_runtime_binding.validate().map_err(|error| {
        QueryPlaybookMaterializationError::new(
            "query-playbook-runtime-binding-mismatch",
            format!("independently admitted Runtime binding is invalid: {error:?}"),
        )
    })?;
    manifest_project_workspace.validate().map_err(|error| {
        QueryPlaybookMaterializationError::new(
            "query-playbook-project-workspace-manifest-mismatch",
            format!("parser-owned manifest Project Workspace is invalid: {error}"),
        )
    })?;
    if expected_runtime_binding.project_workspace != *manifest_project_workspace {
        return invalid(
            "query-playbook-project-workspace-manifest-mismatch",
            "Runtime Project Workspace differs from the parser-owned manifest binding",
        );
    }
    Ok(())
}

fn validate_complete_materializations(
    receipt: &Map<String, Value>,
    request: &Map<String, Value>,
) -> Result<(), QueryPlaybookMaterializationError> {
    let requested = array(request, "selectors")?;
    let materializations = array(receipt, "materializations")?;
    let projection = text(request, "projection")?;
    if materializations.len() != requested.len() {
        return invalid(
            "query-playbook-materialization-set-mismatch",
            "Ready receipt must materialize every requested selector exactly once",
        );
    }
    for (materialization, selector) in materializations.iter().zip(requested) {
        let materialization = object(materialization, "materialization")?;
        if materialization.contains_key("gqlRelationships") {
            return invalid(
                "schema-invalid",
                "Query materialization must not carry Search GQL relationships",
            );
        }
        if materialization.get("selector") != Some(selector)
            || text(materialization, "projection")? != projection
        {
            return invalid(
                "query-playbook-materialization-set-mismatch",
                "Ready materializations must preserve request selector order and projection",
            );
        }
        for field in ["languageId", "providerId", "ownerPath"] {
            text(materialization, field)?;
        }
        let digest = text(materialization, "sourceContentDigest")?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return invalid(
                "schema-invalid",
                "sourceContentDigest must be 64 hex digits",
            );
        }
        let bytes = array(materialization, "bytes")?;
        if bytes.is_empty()
            || bytes
                .iter()
                .any(|byte| byte.as_u64().is_none_or(|value| value > u8::MAX.into()))
        {
            return invalid(
                "schema-invalid",
                "materialized bytes must be non-empty octets",
            );
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryPlaybookMaterializationError {
    reason_kind: &'static str,
    message: String,
}

impl QueryPlaybookMaterializationError {
    fn new(reason_kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason_kind,
            message: message.into(),
        }
    }

    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for QueryPlaybookMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for QueryPlaybookMaterializationError {}

fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> QueryPlaybookMaterializationError {
    QueryPlaybookMaterializationError::new(reason_kind, message)
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, QueryPlaybookMaterializationError> {
    Err(error(reason_kind, message))
}

fn object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, QueryPlaybookMaterializationError> {
    value
        .as_object()
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

fn text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, QueryPlaybookMaterializationError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error("schema-invalid", format!("{field} must be non-empty text")))
}

fn text_eq(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), QueryPlaybookMaterializationError> {
    if text(object, field)? == expected {
        Ok(())
    } else {
        invalid(
            "schema-invalid",
            format!("{field} has an unsupported value"),
        )
    }
}

fn array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, QueryPlaybookMaterializationError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an array")))
}
