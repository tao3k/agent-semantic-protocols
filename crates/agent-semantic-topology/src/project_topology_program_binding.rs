// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! MRR identity and compilation-receipt refinement for Project Topology programs.

use std::collections::BTreeMap;
use std::fmt;

use agent_semantic_content_identity::ProjectWorkspaceBinding;
use meta_relational_reasoning::ReasoningBundleId;
use serde_json::{Map, Value};

const SCHEMA_ID: &str = "agent.semantic-protocols.project-topology-program-binding";
const SCHEMA_VERSION: &str = "1";
const COMPILATION_SCHEMA_ID: &str = "agent.semantic-protocols.mrr-program-compilation-receipt";

/// Fail-closed semantic refinement errors after JSON Schema validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyProgramBindingError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologyProgramBindingError {
    #[must_use]
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectTopologyProgramBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologyProgramBindingError {}

/// Admit the exact MRR bundle identity bound by an independently admitted
/// compilation receipt.
///
/// JSON Schema owns closed transport shape. This refinement owns the semantic
/// equality and MRR domain checks which a generic digest regex cannot prove.
pub fn admit_project_topology_program_bundle(
    packet: &Value,
    manifest_project_workspace: &ProjectWorkspaceBinding,
    admitted_receipts: &BTreeMap<String, Value>,
) -> Result<ReasoningBundleId, ProjectTopologyProgramBindingError> {
    let packet = object(packet, "packet")?;
    require_text(packet, "schemaId", SCHEMA_ID)?;
    require_text(packet, "schemaVersion", SCHEMA_VERSION)?;

    let binding = object_field(packet, "binding")?;
    let encoded_bundle = text(binding, "mrrBundleIdentity")?;
    let bundle_identity = encoded_bundle
        .parse::<ReasoningBundleId>()
        .map_err(|error| {
            invalid(
                "topology-mrr-bundle-identity-invalid",
                format!("mrrBundleIdentity is not a ReasoningBundleId: {error}"),
            )
        })?;

    let program = object_field(packet, "program")?;
    let receipt = object_field(program, "compilationReceipt")?;
    require_text(receipt, "schemaId", COMPILATION_SCHEMA_ID)?;
    require_text(receipt, "schemaVersion", SCHEMA_VERSION)?;
    require_text(receipt, "state", "admitted")?;
    let receipt_id = text(receipt, "id")?;
    if admitted_receipts.get(receipt_id) != Some(&Value::Object(receipt.clone())) {
        return Err(invalid(
            "topology-compilation-receipt-unadmitted",
            "embedded compilation receipt is not independently admitted",
        ));
    }
    for field in [
        "schemeProgramDigest",
        "compiledProgramAbiDigest",
        "mrrBundleIdentity",
    ] {
        if receipt.get(field) != binding.get(field) {
            return Err(invalid(
                "topology-compilation-receipt-mismatch",
                format!("compilation receipt does not bind {field}"),
            ));
        }
    }

    let project_workspace = object_field(binding, "projectWorkspace")?;
    let decoded_project_workspace: ProjectWorkspaceBinding =
        serde_json::from_value(Value::Object(project_workspace.clone())).map_err(|error| {
            invalid(
                "topology-project-workspace-invalid",
                format!("projectWorkspace cannot be decoded: {error}"),
            )
        })?;
    decoded_project_workspace
        .validate()
        .map_err(|error| invalid("topology-project-workspace-invalid", error.to_string()))?;
    if &decoded_project_workspace != manifest_project_workspace {
        return Err(invalid(
            "topology-project-workspace-mismatch",
            "program binding does not equal the parser-owned manifest binding",
        ));
    }

    let terminal = object_field(packet, "terminal")?;
    require_text(terminal, "state", "admitted")?;
    if terminal.get("reasonKind") != Some(&Value::Null) {
        return Err(invalid(
            "topology-admitted-terminal-has-failure-reason",
            "an admitted terminal must have null reasonKind",
        ));
    }

    Ok(bundle_identity)
}

fn object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, ProjectTopologyProgramBindingError> {
    value.as_object().ok_or_else(|| {
        invalid(
            "topology-program-binding-invalid",
            format!("{field} must be an object"),
        )
    })
}

fn object_field<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, ProjectTopologyProgramBindingError> {
    value
        .get(field)
        .ok_or_else(|| {
            invalid(
                "topology-program-binding-invalid",
                format!("missing {field}"),
            )
        })
        .and_then(|value| object(value, field))
}

fn text<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, ProjectTopologyProgramBindingError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        invalid(
            "topology-program-binding-invalid",
            format!("{field} must be a string"),
        )
    })
}

fn require_text(
    value: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), ProjectTopologyProgramBindingError> {
    if text(value, field)? == expected {
        Ok(())
    } else {
        Err(invalid(
            "topology-program-binding-invalid",
            format!("{field} does not match {expected}"),
        ))
    }
}

fn invalid(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> ProjectTopologyProgramBindingError {
    ProjectTopologyProgramBindingError {
        reason_kind,
        message: message.into(),
    }
}
