// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable join between one Runtime Search generation and Project Topology.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use agent_semantic_content_identity::Blake3DigestV1;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use serde_json::{Map, Value};

use crate::ProjectTopologyLibrary;

pub const RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-project-topology-attachment";
pub const RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_VERSION: &str = "1";

/// One admitted, immutable topology authority attached to a Runtime generation.
#[derive(Clone, Debug)]
pub struct RuntimeProjectTopologyAttachment {
    packet: Value,
    runtime_generation_digest: Blake3DigestV1,
    runtime_execution_binding: RuntimeExecutionBinding,
    library: Arc<ProjectTopologyLibrary>,
}

impl RuntimeProjectTopologyAttachment {
    /// Admits the complete attachment product against independent Runtime and
    /// inference-receipt authorities.
    pub fn admit(
        packet: Value,
        expected_runtime_generation_digest: Blake3DigestV1,
        expected_runtime_execution_binding: RuntimeExecutionBinding,
        library: Arc<ProjectTopologyLibrary>,
        independently_admitted_receipts: &BTreeMap<String, Value>,
    ) -> Result<Self, RuntimeProjectTopologyAttachmentError> {
        expected_runtime_execution_binding
            .validate()
            .map_err(|error| {
                invalid(
                    "runtime-project-topology-runtime-binding-invalid",
                    format!("Runtime execution binding is invalid: {error:?}"),
                )
            })?;
        require_digest(
            "runtimeGenerationDigest",
            expected_runtime_generation_digest.as_str(),
        )?;

        let object = object(&packet, "attachment")?;
        require_text_eq(
            object,
            "schemaId",
            RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_ID,
        )?;
        require_text_eq(
            object,
            "schemaVersion",
            RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_VERSION,
        )?;

        let packet_runtime: RuntimeExecutionBinding = serde_json::from_value(
            object
                .get("runtimeExecutionBinding")
                .cloned()
                .ok_or_else(|| schema("runtimeExecutionBinding is required"))?,
        )
        .map_err(|error| schema(format!("runtimeExecutionBinding is invalid: {error}")))?;
        if packet_runtime != expected_runtime_execution_binding
            || text(object, "runtimeGenerationDigest")?
                != expected_runtime_generation_digest.as_str()
        {
            return Err(binding_mismatch(
                "attachment does not equal the independently supplied Runtime generation",
            ));
        }

        let topology = object_field(object, "topologyLibraryBinding")?;
        let project_workspace =
            serde_json::to_value(&expected_runtime_execution_binding.project_workspace)
                .expect("ProjectWorkspaceBinding serializes");
        if topology.get("projectWorkspace") != Some(&project_workspace)
            || expected_runtime_execution_binding.project_workspace != *library.project_workspace()
        {
            return Err(binding_mismatch(
                "Project Workspace binding differs across Runtime and topology library",
            ));
        }

        let content = &expected_runtime_execution_binding.content_binding.identity;
        require_same(
            topology,
            "sourceGenerationDigest",
            &content.source_generation_digest,
        )?;
        require_same(
            topology,
            "providerCatalogDigest",
            &content.provider_catalog_digest,
        )?;
        require_same(
            topology,
            "sourceGenerationDigest",
            library.source_generation_digest(),
        )?;
        require_same(
            topology,
            "providerCatalogDigest",
            library.provider_catalog_digest(),
        )?;
        require_same(topology, "libraryDigest", library.library_digest())?;
        require_same(
            topology,
            "topologyGenerationDigest",
            library.generation_digest(),
        )?;
        require_same(
            topology,
            "structuralTopologyDigest",
            library.structural_topology_digest(),
        )?;
        require_same(
            topology,
            "semanticTopologyDigest",
            library.semantic_topology_digest(),
        )?;
        require_same(
            topology,
            "inferenceProgramDigest",
            library.inference_program_digest(),
        )?;
        require_same(topology, "closureDigest", library.closure_digest())?;
        require_digest(
            "parserCatalogDigest",
            text(topology, "parserCatalogDigest")?,
        )?;

        let receipt = object_field(object, "inferenceReceipt")?;
        require_text_eq(receipt, "state", "admitted")?;
        let receipt_digest = text(receipt, "receiptDigest")?;
        require_digest("receiptDigest", receipt_digest)?;
        if independently_admitted_receipts.get(receipt_digest)
            != Some(&Value::Object(receipt.clone()))
        {
            return Err(invalid(
                "runtime-topology-inference-receipt-unadmitted",
                "embedded inference receipt is not independently admitted",
            ));
        }
        for field in [
            "sourceGenerationDigest",
            "providerCatalogDigest",
            "parserCatalogDigest",
            "libraryDigest",
            "topologyGenerationDigest",
            "inferenceProgramDigest",
            "closureDigest",
        ] {
            if receipt.get(field) != topology.get(field) {
                return Err(binding_mismatch(format!(
                    "inference receipt does not bind {field}"
                )));
            }
        }
        if receipt.get("projectWorkspace") != topology.get("projectWorkspace")
            || text(receipt, "runtimeGenerationDigest")?
                != expected_runtime_generation_digest.as_str()
            || text(receipt, "runtimeArtifactDigest")?
                != expected_runtime_execution_binding
                    .runtime_artifact_digest
                    .as_str()
        {
            return Err(binding_mismatch(
                "inference receipt does not bind the exact Runtime product",
            ));
        }

        let terminal = object_field(object, "terminal")?;
        if text(terminal, "state")? != "admitted"
            || terminal.get("terminalCount").and_then(Value::as_u64) != Some(1)
            || terminal.get("reasonKind") != Some(&Value::Null)
        {
            return Err(invalid(
                "runtime-project-topology-terminal-invalid",
                "attachment requires exactly one admitted terminal with no failure reason",
            ));
        }

        Ok(Self {
            packet,
            runtime_generation_digest: expected_runtime_generation_digest,
            runtime_execution_binding: expected_runtime_execution_binding,
            library,
        })
    }

    #[must_use]
    pub fn runtime_generation_digest(&self) -> &str {
        self.runtime_generation_digest.as_str()
    }

    #[must_use]
    pub fn runtime_execution_binding(&self) -> &RuntimeExecutionBinding {
        &self.runtime_execution_binding
    }

    #[must_use]
    pub fn library(&self) -> &ProjectTopologyLibrary {
        &self.library
    }

    #[must_use]
    pub fn as_json(&self) -> &Value {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectTopologyAttachmentError {
    reason_kind: &'static str,
    message: String,
}

impl RuntimeProjectTopologyAttachmentError {
    #[must_use]
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for RuntimeProjectTopologyAttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for RuntimeProjectTopologyAttachmentError {}

fn object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, RuntimeProjectTopologyAttachmentError> {
    value
        .as_object()
        .ok_or_else(|| schema(format!("{field} must be an object")))
}

fn object_field<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, RuntimeProjectTopologyAttachmentError> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| schema(format!("{field} must be an object")))
}

fn text<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, RuntimeProjectTopologyAttachmentError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| schema(format!("{field} must be non-empty text")))
}

fn require_text_eq(
    value: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), RuntimeProjectTopologyAttachmentError> {
    if text(value, field)? != expected {
        return Err(schema(format!("{field} does not match {expected}")));
    }
    Ok(())
}

fn require_same(
    value: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), RuntimeProjectTopologyAttachmentError> {
    let observed = text(value, field)?;
    require_digest(field, observed)?;
    if observed != expected {
        return Err(binding_mismatch(format!("{field} does not match")));
    }
    Ok(())
}

fn require_digest(field: &str, digest: &str) -> Result<(), RuntimeProjectTopologyAttachmentError> {
    if digest.len() != "blake3-256:".len() + 64
        || !digest.starts_with("blake3-256:")
        || !digest["blake3-256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(schema(format!("{field} must be a canonical BLAKE3 digest")));
    }
    Ok(())
}

fn invalid(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> RuntimeProjectTopologyAttachmentError {
    RuntimeProjectTopologyAttachmentError {
        reason_kind,
        message: message.into(),
    }
}

fn schema(message: impl Into<String>) -> RuntimeProjectTopologyAttachmentError {
    invalid("runtime-project-topology-schema-invalid", message)
}

fn binding_mismatch(message: impl Into<String>) -> RuntimeProjectTopologyAttachmentError {
    invalid("runtime-project-topology-binding-mismatch", message)
}
