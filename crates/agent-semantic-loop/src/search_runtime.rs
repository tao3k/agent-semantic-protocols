// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;
use std::fmt;

use agent_semantic_context_product::Digest;
use agent_semantic_context_product::ProtocolId;
use serde::Deserialize;
use serde::Serialize;

pub const SEARCH_LOOP_OPEN_ENVELOPE_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-loop-open-envelope";
pub const SEARCH_LOOP_RUNTIME_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-loop-runtime-binding";
pub const SEARCH_LOOP_RUNTIME_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLoopArtifactBindingV1 {
    artifact_ref: ProtocolId,
    artifact_digest: Digest,
    artifact_schema_id: ProtocolId,
}

impl SearchLoopArtifactBindingV1 {
    pub fn artifact_ref(&self) -> &ProtocolId {
        &self.artifact_ref
    }

    pub fn artifact_digest(&self) -> &Digest {
        &self.artifact_digest
    }

    pub fn artifact_schema_id(&self) -> &ProtocolId {
        &self.artifact_schema_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedSearchLoopOpenEnvelopeV1 {
    schema_id: String,
    schema_version: String,
    dispatch_ref: ProtocolId,
    dispatch_receipt_digest: Digest,
    resident_identity_ref: ProtocolId,
    language_id: String,
    context_binding_digest: Digest,
    context_state_artifact: SearchLoopArtifactBindingV1,
    authority_receipt_artifact: SearchLoopArtifactBindingV1,
    proposal_set_artifact: SearchLoopArtifactBindingV1,
    choice_panel_artifact: SearchLoopArtifactBindingV1,
    graph_cursor_artifact: SearchLoopArtifactBindingV1,
    issued_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopOpenEnvelopeV1(UncheckedSearchLoopOpenEnvelopeV1);

impl SearchLoopOpenEnvelopeV1 {
    pub fn validate(
        unchecked: UncheckedSearchLoopOpenEnvelopeV1,
    ) -> Result<Self, SearchLoopRuntimeValidationError> {
        if unchecked.schema_id != SEARCH_LOOP_OPEN_ENVELOPE_SCHEMA_ID
            || unchecked.schema_version != SEARCH_LOOP_RUNTIME_SCHEMA_VERSION
        {
            return Err(SearchLoopRuntimeValidationError::SchemaIdentity);
        }
        if !is_language_id(&unchecked.language_id) {
            return Err(SearchLoopRuntimeValidationError::LanguageId);
        }
        validate_graph_cursor_artifact(&unchecked.graph_cursor_artifact)?;
        Ok(Self(unchecked))
    }

    pub fn dispatch_ref(&self) -> &ProtocolId {
        &self.0.dispatch_ref
    }

    pub fn dispatch_receipt_digest(&self) -> &Digest {
        &self.0.dispatch_receipt_digest
    }

    pub fn resident_identity_ref(&self) -> &ProtocolId {
        &self.0.resident_identity_ref
    }

    pub fn language_id(&self) -> &str {
        &self.0.language_id
    }

    pub fn context_binding_digest(&self) -> &Digest {
        &self.0.context_binding_digest
    }

    pub fn context_state_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.0.context_state_artifact
    }

    pub fn authority_receipt_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.0.authority_receipt_artifact
    }

    pub fn proposal_set_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.0.proposal_set_artifact
    }

    pub fn choice_panel_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.0.choice_panel_artifact
    }

    pub fn graph_cursor_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.0.graph_cursor_artifact
    }

    pub fn issued_at_unix_ms(&self) -> u64 {
        self.0.issued_at_unix_ms
    }

    pub fn into_unchecked(self) -> UncheckedSearchLoopOpenEnvelopeV1 {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLoopActivePanelBindingV1 {
    panel_id: ProtocolId,
    panel_artifact: SearchLoopArtifactBindingV1,
    proposal_set_artifact: SearchLoopArtifactBindingV1,
    graph_cursor_artifact: SearchLoopArtifactBindingV1,
    based_on_revision: u64,
}

impl SearchLoopActivePanelBindingV1 {
    pub fn panel_id(&self) -> &ProtocolId {
        &self.panel_id
    }

    pub fn panel_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.panel_artifact
    }

    pub fn proposal_set_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.proposal_set_artifact
    }

    pub fn graph_cursor_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.graph_cursor_artifact
    }

    pub fn based_on_revision(&self) -> u64 {
        self.based_on_revision
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchLoopBatchStatus {
    Active,
    Joined,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLoopBatchBindingV1 {
    batch_id: ProtocolId,
    execution_group_ref: ProtocolId,
    program_ref: ProtocolId,
    program_digest: Digest,
    view_artifact: SearchLoopArtifactBindingV1,
    based_on_revision: u64,
    status: SearchLoopBatchStatus,
}

impl SearchLoopBatchBindingV1 {
    pub fn batch_id(&self) -> &ProtocolId {
        &self.batch_id
    }

    pub fn execution_group_ref(&self) -> &ProtocolId {
        &self.execution_group_ref
    }

    pub fn program_ref(&self) -> &ProtocolId {
        &self.program_ref
    }

    pub fn program_digest(&self) -> &Digest {
        &self.program_digest
    }

    pub fn view_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.view_artifact
    }

    pub fn based_on_revision(&self) -> u64 {
        self.based_on_revision
    }

    pub fn status(&self) -> SearchLoopBatchStatus {
        self.status
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLoopTerminalBindingV1 {
    closure_receipt_ref: ProtocolId,
    closure_receipt_digest: Digest,
    query_handoff_artifact: SearchLoopArtifactBindingV1,
    finalized_at_revision: u64,
}

impl SearchLoopTerminalBindingV1 {
    pub fn closure_receipt_ref(&self) -> &ProtocolId {
        &self.closure_receipt_ref
    }

    pub fn closure_receipt_digest(&self) -> &Digest {
        &self.closure_receipt_digest
    }

    pub fn query_handoff_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.query_handoff_artifact
    }

    pub fn finalized_at_revision(&self) -> u64 {
        self.finalized_at_revision
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedSearchLoopRuntimeBindingV1 {
    schema_id: String,
    schema_version: String,
    loop_id: ProtocolId,
    run_id: ProtocolId,
    resident_identity_ref: ProtocolId,
    dispatch_ref: ProtocolId,
    dispatch_receipt_digest: Digest,
    context_binding_digest: Digest,
    opened_at_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_panel: Option<SearchLoopActivePanelBindingV1>,
    batches: Vec<SearchLoopBatchBindingV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_binding: Option<SearchLoopTerminalBindingV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopRuntimeBindingV1(UncheckedSearchLoopRuntimeBindingV1);

impl SearchLoopRuntimeBindingV1 {
    pub fn validate(
        unchecked: UncheckedSearchLoopRuntimeBindingV1,
    ) -> Result<Self, SearchLoopRuntimeValidationError> {
        if unchecked.schema_id != SEARCH_LOOP_RUNTIME_BINDING_SCHEMA_ID
            || unchecked.schema_version != SEARCH_LOOP_RUNTIME_SCHEMA_VERSION
        {
            return Err(SearchLoopRuntimeValidationError::SchemaIdentity);
        }
        if unchecked.loop_id == unchecked.run_id {
            return Err(SearchLoopRuntimeValidationError::LoopRunAlias);
        }
        match (&unchecked.active_panel, &unchecked.terminal_binding) {
            (Some(_), Some(_)) => {
                return Err(SearchLoopRuntimeValidationError::TerminalHasActivePanel);
            }
            (None, None) => {
                return Err(SearchLoopRuntimeValidationError::ActivePanelMissing);
            }
            _ => {}
        }
        if unchecked
            .active_panel
            .as_ref()
            .is_some_and(|panel| panel.based_on_revision < unchecked.opened_at_revision)
        {
            return Err(SearchLoopRuntimeValidationError::RevisionBeforeOpen);
        }
        if let Some(panel) = &unchecked.active_panel {
            validate_graph_cursor_artifact(&panel.graph_cursor_artifact)?;
        }
        if unchecked
            .terminal_binding
            .as_ref()
            .is_some_and(|terminal| terminal.finalized_at_revision < unchecked.opened_at_revision)
        {
            return Err(SearchLoopRuntimeValidationError::RevisionBeforeOpen);
        }

        let mut batch_ids = BTreeSet::new();
        let mut execution_groups = BTreeSet::new();
        for batch in &unchecked.batches {
            if batch.based_on_revision < unchecked.opened_at_revision {
                return Err(SearchLoopRuntimeValidationError::RevisionBeforeOpen);
            }
            if !batch_ids.insert(batch.batch_id.clone()) {
                return Err(SearchLoopRuntimeValidationError::DuplicateBatchId);
            }
            if !execution_groups.insert(batch.execution_group_ref.clone()) {
                return Err(SearchLoopRuntimeValidationError::DuplicateExecutionGroup);
            }
        }

        Ok(Self(unchecked))
    }

    pub fn loop_id(&self) -> &ProtocolId {
        &self.0.loop_id
    }

    pub fn run_id(&self) -> &ProtocolId {
        &self.0.run_id
    }

    pub fn resident_identity_ref(&self) -> &ProtocolId {
        &self.0.resident_identity_ref
    }

    pub fn dispatch_ref(&self) -> &ProtocolId {
        &self.0.dispatch_ref
    }

    pub fn dispatch_receipt_digest(&self) -> &Digest {
        &self.0.dispatch_receipt_digest
    }

    pub fn context_binding_digest(&self) -> &Digest {
        &self.0.context_binding_digest
    }

    pub fn opened_at_revision(&self) -> u64 {
        self.0.opened_at_revision
    }

    pub fn active_panel(&self) -> Option<&SearchLoopActivePanelBindingV1> {
        self.0.active_panel.as_ref()
    }

    pub fn batches(&self) -> &[SearchLoopBatchBindingV1] {
        &self.0.batches
    }

    pub fn terminal_binding(&self) -> Option<&SearchLoopTerminalBindingV1> {
        self.0.terminal_binding.as_ref()
    }

    pub fn validate_active_graph_cursor(
        &self,
        cursor: &crate::search_graph_cursor::SearchGraphCursorArtifact,
    ) -> Result<(), SearchLoopRuntimeValidationError> {
        if self.active_panel().is_none() {
            return Err(SearchLoopRuntimeValidationError::ActiveGraphCursorMissing);
        }
        if cursor.loop_id() != self.loop_id() {
            return Err(SearchLoopRuntimeValidationError::GraphCursorLoopMismatch);
        }
        Ok(())
    }

    pub fn into_unchecked(self) -> UncheckedSearchLoopRuntimeBindingV1 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchLoopRuntimeValidationError {
    SchemaIdentity,
    LanguageId,
    LoopRunAlias,
    TerminalHasActivePanel,
    ActivePanelMissing,
    RevisionBeforeOpen,
    DuplicateBatchId,
    DuplicateExecutionGroup,
    GraphCursorArtifactSchema,
    ActiveGraphCursorMissing,
    GraphCursorLoopMismatch,
}

impl fmt::Display for SearchLoopRuntimeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SchemaIdentity => "invalid SearchLoop runtime schema identity",
            Self::LanguageId => "invalid SearchLoop language id",
            Self::LoopRunAlias => "loop id and run id must be distinct typed identities",
            Self::TerminalHasActivePanel => "terminal SearchLoop retains an active panel",
            Self::ActivePanelMissing => "active SearchLoop is missing its panel binding",
            Self::RevisionBeforeOpen => {
                "runtime binding refers to a revision before the loop opened"
            }
            Self::DuplicateBatchId => "runtime binding contains a duplicate batch id",
            Self::DuplicateExecutionGroup => "runtime binding contains a duplicate execution group",
            Self::GraphCursorArtifactSchema => {
                "SearchLoop cursor artifact has the wrong schema identity"
            }
            Self::ActiveGraphCursorMissing => {
                "terminal SearchLoop cannot admit an active graph cursor"
            }
            Self::GraphCursorLoopMismatch => {
                "SearchLoop cursor artifact belongs to a different loop"
            }
        })
    }
}

impl std::error::Error for SearchLoopRuntimeValidationError {}

fn is_language_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_graph_cursor_artifact(
    artifact: &SearchLoopArtifactBindingV1,
) -> Result<(), SearchLoopRuntimeValidationError> {
    if artifact.artifact_schema_id().as_str()
        != crate::search_graph_cursor::SEARCH_GRAPH_CURSOR_SCHEMA_ID
    {
        return Err(SearchLoopRuntimeValidationError::GraphCursorArtifactSchema);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/search_runtime.rs"]
mod tests;
