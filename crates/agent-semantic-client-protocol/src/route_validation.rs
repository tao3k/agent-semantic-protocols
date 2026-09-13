// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation logic for typed ASP Client Protocol routes.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::routes::{
    AspClientExactQueryFailure, AspClientExactQueryRequest, AspClientExactQueryResponse,
    AspClientGraphsTimelineRequest, AspClientSearchPlaybookClauseAxis,
    AspClientSourceIndexLookupRequest, AspClientWorkspaceQueryPlaybookRequest,
    AspClientWorkspaceSearchPlaybookRequest, AspClientWorkspaceSyntaxPlanContextResponse,
    AspClientWorkspaceSyntaxQueryRequest, CLIENT_EXACT_QUERY_FAILURE, CLIENT_EXACT_QUERY_REQUEST,
    CLIENT_EXACT_QUERY_RESPONSE, CLIENT_GRAPHS_TIMELINE_REQUEST,
    CLIENT_SOURCE_INDEX_LOOKUP_REQUEST, CLIENT_WORKSPACE_QUERY_PLAYBOOK_REQUEST,
    CLIENT_WORKSPACE_SEARCH_PLAYBOOK_REQUEST, CLIENT_WORKSPACE_SYNTAX_PLAN_CONTEXT_REQUEST,
    CLIENT_WORKSPACE_SYNTAX_PLAN_CONTEXT_RESPONSE, CLIENT_WORKSPACE_SYNTAX_QUERY_REQUEST,
    EXACT_REQUEST, EXACT_RESPONSE, ProviderNativeExactProjection, ProviderNativeExactRequest,
    RUNTIME_RESIDENT_REQUEST_PLANE_RECEIPT_SCHEMA_ID, RuntimeProviderSearchRequest,
    RuntimeResidentRequestPlaneReceipt, SEARCH_REQUEST,
};

fn check(id: &str, schema_id: &str, schema_version: &str) -> Result<(), String> {
    check_version(id, schema_id, schema_version, "1")
}

fn check_version(
    id: &str,
    schema_id: &str,
    schema_version: &str,
    expected_version: &str,
) -> Result<(), String> {
    if id != schema_id {
        return Err(format!(
            "route schema identity drift: expected={schema_id} actual={id}"
        ));
    }
    if schema_version != expected_version {
        return Err(format!(
            "route schema version unsupported: {schema_version}"
        ));
    }
    Ok(())
}

macro_rules! validate_schema_identity {
    ($name:ident, $id:expr) => {
        impl $name {
            pub fn validate_schema_identity(&self) -> Result<(), String> {
                check(&self.schema_id, $id, &self.schema_version)
            }
        }
    };
}
validate_schema_identity!(
    AspClientSourceIndexLookupRequest,
    CLIENT_SOURCE_INDEX_LOOKUP_REQUEST
);
validate_schema_identity!(AspClientExactQueryRequest, CLIENT_EXACT_QUERY_REQUEST);
validate_schema_identity!(
    AspClientGraphsTimelineRequest,
    CLIENT_GRAPHS_TIMELINE_REQUEST
);
validate_schema_identity!(ProviderNativeExactRequest, EXACT_REQUEST);
validate_schema_identity!(ProviderNativeExactProjection, EXACT_RESPONSE);
impl RuntimeProviderSearchRequest {
    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check(&self.schema_id, SEARCH_REQUEST, &self.schema_version)?;
        if self.operation_id.trim().is_empty()
            || self.project_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.language_id.trim().is_empty()
        {
            return Err("Runtime provider Search request identity is incomplete".to_owned());
        }
        Ok(())
    }
}

impl AspClientWorkspaceSearchPlaybookRequest {
    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check_version(
            &self.schema_id,
            CLIENT_WORKSPACE_SEARCH_PLAYBOOK_REQUEST,
            &self.schema_version,
            "1",
        )?;
        if self.language.is_none() && self.documents.is_none() {
            return Err("ASP workspace Search requires language or documents producers".to_owned());
        }
        for (name, value) in [
            ("language", self.language.as_deref()),
            ("documents", self.documents.as_deref()),
            ("workspace", self.workspace.as_deref()),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(format!("ASP workspace Search {name} must not be empty"));
            }
        }

        let acquisition_count = self.rg.as_ref().map_or(0, Vec::len)
            + self.tantivy.as_ref().map_or(0, Vec::len)
            + self.syntax.as_ref().map_or(0, Vec::len)
            + self.native_syntax.as_ref().map_or(0, Vec::len);
        if self.rg.is_none() || self.tantivy.is_none() {
            return Err(
                "ASP workspace Search Layout requires rg and Tantivy file-context inputs"
                    .to_owned(),
            );
        }
        let graph_count = self.graph.as_ref().map_or(0, Vec::len);
        if acquisition_count == 0 && graph_count != 0 {
            return Err("ASP workspace Search Graph requires preceding acquisition".to_owned());
        }
        if acquisition_count == 0 {
            return Err("ASP workspace Search requires an acquisition clause".to_owned());
        }
        let clause_order = &self.clause_order;
        if acquisition_count == 0 || clause_order.len() != acquisition_count + graph_count {
            return Err("ASP workspace Search clauseOrder coverage is invalid".to_owned());
        }
        let mut graph_started = false;
        let mut covered = BTreeSet::new();
        for clause in clause_order {
            let block_count = match clause.axis {
                AspClientSearchPlaybookClauseAxis::Rg => self.rg.as_ref().map_or(0, Vec::len),
                AspClientSearchPlaybookClauseAxis::Tantivy => {
                    self.tantivy.as_ref().map_or(0, Vec::len)
                }
                AspClientSearchPlaybookClauseAxis::Syntax => {
                    self.syntax.as_ref().map_or(0, Vec::len)
                }
                AspClientSearchPlaybookClauseAxis::NativeSyntax => {
                    self.native_syntax.as_ref().map_or(0, Vec::len)
                }
                AspClientSearchPlaybookClauseAxis::Graph => {
                    graph_started = true;
                    graph_count
                }
            };
            if clause.axis != AspClientSearchPlaybookClauseAxis::Graph && graph_started {
                return Err(
                    "ASP workspace Search acquisition clauses must precede Graph".to_owned(),
                );
            }
            if clause.block_index >= block_count
                || !covered.insert((clause.axis, clause.block_index))
            {
                return Err("ASP workspace Search clauseOrder reference is invalid".to_owned());
            }
        }

        for argv in self.rg.iter().chain(self.tantivy.iter()).flatten() {
            if argv.is_empty() {
                return Err("ASP workspace Search native argv must not be empty".to_owned());
            }
        }
        if self.syntax.iter().flatten().any(|block| {
            block.producer.is_empty()
                || block.plan.language_id != block.producer
                || block.plan.validate().is_err()
        }) {
            return Err("ASP workspace Search Syntax query block must not be empty".to_owned());
        }
        if self.native_syntax.iter().flatten().any(|selector| {
            selector.is_empty()
                || selector.contains(char::is_whitespace)
                || !selector.contains("://")
                || !selector.contains("#item/")
        }) {
            return Err(
                "ASP workspace Search nativeSyntax must contain exact selectors".to_owned(),
            );
        }
        if self
            .graph
            .iter()
            .flatten()
            .any(|block| block.language.is_empty() || block.argv.is_empty())
        {
            return Err("ASP workspace Search Graph block must not be empty".to_owned());
        }
        Ok(())
    }
}

impl AspClientWorkspaceQueryPlaybookRequest {
    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check_version(
            &self.schema_id,
            CLIENT_WORKSPACE_QUERY_PLAYBOOK_REQUEST,
            &self.schema_version,
            "1",
        )?;
        let selected = self
            .language
            .iter()
            .chain(self.documents.iter())
            .flat_map(|expression| expression.split('|'))
            .collect::<BTreeSet<_>>();
        if selected.is_empty()
            || selected.iter().any(|producer| producer.is_empty())
            || self.selectors.is_empty()
            || self.selectors.iter().any(|selector| {
                selector.split_once("://").is_none_or(|(producer, rest)| {
                    !selected.contains(producer) || !rest.contains("#item/")
                })
            })
            || self.selectors.iter().collect::<BTreeSet<_>>().len() != self.selectors.len()
        {
            return Err(
                "workspace Query Playbook selectors must be canonical, unique, and declared by language or documents"
                    .to_owned(),
            );
        }
        if !matches!(self.projection.as_str(), "source" | "callable-skeleton") {
            return Err("workspace Query Playbook projection is unsupported".to_owned());
        }
        Ok(())
    }
}

impl AspClientWorkspaceSyntaxQueryRequest {
    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_WORKSPACE_SYNTAX_QUERY_REQUEST,
            &self.schema_version,
        )?;
        if self.syntax.is_empty()
            || self.syntax.iter().any(|block| {
                block.producer.trim().is_empty()
                    || block.plan.language_id != block.producer
                    || block.plan.validate().is_err()
            })
        {
            return Err("workspace syntax Query requires complete resident plans".to_owned());
        }
        if self.projection != "matches" {
            return Err("workspace syntax Query projection must be matches".to_owned());
        }
        Ok(())
    }
}

impl crate::AspClientWorkspaceSyntaxPlanContextRequest {
    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_WORKSPACE_SYNTAX_PLAN_CONTEXT_REQUEST,
            &self.schema_version,
        )?;
        if self.producer.trim().is_empty() {
            return Err("workspace syntax plan context request is incomplete".to_owned());
        }
        Ok(())
    }
}

impl AspClientWorkspaceSyntaxPlanContextResponse {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_WORKSPACE_SYNTAX_PLAN_CONTEXT_RESPONSE,
            &self.schema_version,
        )?;
        validate_digest("generationDigest", &self.generation_digest, true)?;
        self.capability.validate()
    }
}

impl RuntimeResidentRequestPlaneReceipt {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            RUNTIME_RESIDENT_REQUEST_PLANE_RECEIPT_SCHEMA_ID,
            &self.schema_version,
        )?;
        match self.state.as_str() {
            "ready" => validate_digest(
                "generationDigest",
                self.generation_digest.as_deref().ok_or_else(|| {
                    "Ready request-plane receipt requires generationDigest".to_owned()
                })?,
                true,
            )?,
            "query-not-ready" if self.generation_digest.is_none() => {}
            "query-not-ready" => {
                return Err(
                    "query-not-ready request-plane receipt cannot bind a generation".to_owned(),
                );
            }
            _ => return Err("request-plane receipt state is invalid".to_owned()),
        }
        if self.elapsed_micros >= 1_000
            || self.generation_lookup_count != 1
            || self.generation_wait_count != 0
            || self.generation_build_count != 0
            || self.filesystem_read_count != 0
            || self.database_read_count != 0
            || self.provider_process_count != 0
            || self.parser_invocation_count != 0
            || self.secondary_runtime_rpc_count != 0
            || self.socket_discovery_count != 0
            || self.terminal_wait_count != 0
        {
            return Err(
                "request-plane receipt exceeds its strict latency or zero-external-work contract"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl AspClientExactQueryResponse {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_EXACT_QUERY_RESPONSE,
            &self.schema_version,
        )?;
        if self.operation_id.trim().is_empty()
            || self.project_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.provider_id.trim().is_empty()
        {
            return Err("exact-query response identity fields must be non-empty".to_owned());
        }
        validate_digest("generationDigest", &self.generation_digest, true)?;
        validate_digest("rootDigest", &self.root_digest, false)?;
        if !self.result.is_object() {
            return Err("exact-query response result must be an object".to_owned());
        }
        let state = self
            .result
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| "exact-query response result state is missing".to_owned())?;
        match state {
            "projection" | "provider-projection" => {
                let bytes = self
                    .result
                    .get("bytes")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "exact-query Ready result has no byte payload".to_owned())?;
                if bytes.is_empty() {
                    return Err("exact-query Ready result has an empty byte payload".to_owned());
                }
            }
            _ => {
                return Err(format!(
                    "exact-query response result state {state} is not terminal"
                ));
            }
        }
        if self.elapsed_micros
            != self
                .resident_read_elapsed_micros
                .saturating_add(self.service_elapsed_micros)
        {
            return Err("exact-query response elapsedMicros is inconsistent".to_owned());
        }
        Ok(())
    }
}

impl AspClientExactQueryFailure {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_EXACT_QUERY_FAILURE,
            &self.schema_version,
        )?;
        if self.state != "failed" {
            return Err("exact-query failure state must be failed".to_owned());
        }
        if self.operation_id.trim().is_empty()
            || self.project_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.provider_id.trim().is_empty()
            || self.phase.trim().is_empty()
            || self.reason_kind.trim().is_empty()
        {
            return Err("exact-query failure identity fields must be non-empty".to_owned());
        }
        if let Some(generation_digest) = &self.generation_digest {
            validate_digest("generationDigest", generation_digest, true)?;
        }
        if let Some(root_digest) = &self.root_digest {
            validate_digest("rootDigest", root_digest, false)?;
        }
        if !self.details.is_object() {
            return Err("exact-query failure requires object details".to_owned());
        }
        if self.elapsed_micros
            != self
                .resident_read_elapsed_micros
                .saturating_add(self.service_elapsed_micros)
        {
            return Err("exact-query failure elapsedMicros is inconsistent".to_owned());
        }
        Ok(())
    }
}

fn validate_digest(field: &str, value: &str, algorithm_prefix: bool) -> Result<(), String> {
    let hex = if algorithm_prefix {
        value
            .strip_prefix("blake3-256:")
            .ok_or_else(|| format!("exact-query response {field} uses an unsupported digest"))?
    } else {
        value
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("exact-query response {field} is invalid"));
    }
    Ok(())
}
