// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed provider route request and response bindings for the ASP Client Protocol.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const SEARCH_REQUEST: &str = "agent.semantic-protocols.runtime-provider-search-request";
const CLIENT_SEARCH_REQUEST: &str = "agent.semantic-protocols.asp-client-search-request";
const CLIENT_WORKSPACE_SEARCH_PLAYBOOK_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-search-playbook-request";
const CLIENT_WORKSPACE_QUERY_PLAYBOOK_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-query-playbook-request";
const CLIENT_WORKSPACE_SYNTAX_QUERY_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-query-request";
const CLIENT_SOURCE_INDEX_LOOKUP_REQUEST: &str =
    "agent.semantic-protocols.asp-client-source-index-lookup-request";
const CLIENT_EXACT_QUERY_REQUEST: &str = "agent.semantic-protocols.asp-client-exact-query-request";
const CLIENT_EXACT_QUERY_RESPONSE: &str =
    "agent.semantic-protocols.asp-client-exact-query-response";
const CLIENT_EXACT_QUERY_FAILURE: &str = "agent.semantic-protocols.asp-client-exact-query-failure";
const CLIENT_GRAPHS_TIMELINE_REQUEST: &str =
    "agent.semantic-protocols.asp-client-graphs-timeline-request";
/// Schema identity for live-corpus cache-state requests.
pub const LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.live-corpus-cache-state-request";
/// Schema identity for live-corpus cache-state receipts.
pub const LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.live-corpus-cache-state-receipt";
const EXACT_REQUEST: &str = "agent.semantic-protocols.provider-native-exact-request";
const EXACT_RESPONSE: &str = "agent.semantic-protocols.provider-native-exact-projection";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Conceptual search request submitted through the Runtime client protocol.
pub struct AspClientSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub intent: String,
    pub query: String,
    pub scope: String,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
    pub explain: String,
}

/// One provider-native Syntax block inside Search Playbook.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientSearchPlaybookSyntaxBlock {
    pub producer: String,
    pub argv: Vec<String>,
}

/// One native Graph-language block inside Search Playbook.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientSearchPlaybookGraphBlock {
    pub language: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AspClientSearchPlaybookClauseAxis {
    Fd,
    Rg,
    Tantivy,
    Syntax,
    #[serde(rename = "native-syntax")]
    NativeSyntax,
    Graph,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientSearchPlaybookClauseRef {
    pub axis: AspClientSearchPlaybookClauseAxis,
    pub block_index: usize,
}

/// Workspace-scoped Search Playbook request owned by the Runtime Server.
///
/// One or more acquisition clauses form an executable request; optional Graph
/// clauses are the final dependent fan-in barrier. Incomplete input fails
/// before Runtime dispatch. No contract-query mode or intent field exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceSearchPlaybookRequest {
    pub schema_id: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fd: Option<Vec<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rg: Option<Vec<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tantivy: Option<Vec<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<Vec<AspClientSearchPlaybookSyntaxBlock>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_syntax: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<Vec<AspClientSearchPlaybookGraphBlock>>,
    pub clause_order: Vec<AspClientSearchPlaybookClauseRef>,
}

/// One language-neutral Query Playbook request submitted to the Runtime Server.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceQueryPlaybookRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub selectors: Vec<String>,
    pub projection: String,
}

/// Direct provider-native syntax Query over the current immutable workspace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceSyntaxQueryRequest {
    pub schema_id: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    pub syntax: Vec<AspClientSearchPlaybookSyntaxBlock>,
    pub projection: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceSyntaxQueryEvidence {
    pub owner: String,
    pub selector: String,
    pub relation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceSyntaxQueryResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub evidence: Vec<AspClientWorkspaceSyntaxQueryEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Lookup request against a published Source Index generation.
pub struct AspClientSourceIndexLookupRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub query: String,
    pub index_root: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Exact query request addressed by a provider-owned structural selector.
pub struct AspClientExactQueryRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub selector: String,
    pub projection: String,
}

/// Server-owned graph timeline request used by history/audit clients.
///
/// Graph-Turbo is an algorithm identity carried by the event packet; the
/// transport and lifecycle owner is ASP Server's `asp-python-graphs` service.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Request to evaluate a graph timeline from a typed event packet.
pub struct AspClientGraphsTimelineRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub event_packet: Value,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Request to inspect or mutate the Runtime-owned live-corpus cache state.
pub struct LiveCorpusCacheStateRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub resource_id: String,
    pub language_id: String,
    pub provider_id: String,
    pub artifact_digest: String,
    pub cache_state: String,
    pub prepare_action: String,
    pub mutation_scope: String,
    pub expected_generation_digest: Option<String>,
    pub expected_root_digest: Option<String>,
}

impl LiveCorpusCacheStateRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID
            || self.schema_version != "1"
            || self.operation_id.is_empty()
            || self.resource_id.is_empty()
            || self.language_id.is_empty()
            || self.provider_id.is_empty()
            || !valid_hex_digest(&self.artifact_digest)
        {
            return Err("Live Corpus cache-state request identity is invalid".to_owned());
        }
        let exact_generation = self
            .expected_generation_digest
            .as_deref()
            .is_some_and(valid_blake3_digest);
        let exact_root = self
            .expected_root_digest
            .as_deref()
            .is_some_and(valid_hex_digest);
        match self.cache_state.as_str() {
            "cold-build" => {
                if self.prepare_action != "new-isolated-workspace-generation"
                    || self.mutation_scope != "benchmark-workspace-generation"
                    || self.expected_generation_digest.is_some()
                    || self.expected_root_digest.is_some()
                {
                    return Err("Live Corpus cold-build request is not isolated".to_owned());
                }
            }
            "cold-load" => {
                if self.prepare_action != "evict-resident-generation-only"
                    || self.mutation_scope != "benchmark-workspace-generation"
                    || !exact_generation
                    || !exact_root
                {
                    return Err("Live Corpus cold-load request is not content-bound".to_owned());
                }
            }
            "warm-read" => {
                if self.prepare_action != "reuse-exact-resident-generation"
                    || self.mutation_scope != "none"
                    || !exact_generation
                    || !exact_root
                {
                    return Err("Live Corpus warm-read request is not content-bound".to_owned());
                }
            }
            "released" => {
                if self.prepare_action != "release-exact-benchmark-generation"
                    || self.mutation_scope != "benchmark-workspace-generation"
                    || !exact_generation
                    || !exact_root
                {
                    return Err("Live Corpus release request is not content-bound".to_owned());
                }
            }
            _ => return Err("Live Corpus cache state is unsupported".to_owned()),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Terminal receipt for a live-corpus cache-state operation.
pub struct LiveCorpusCacheStateReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub state: String,
    pub cache_state: String,
    pub project_id: String,
    pub workspace_id: String,
    pub generation_digest: Option<String>,
    pub root_digest: Option<String>,
    pub resident_generation_evicted: bool,
    pub client_session_evicted: bool,
    pub source_workspace_mutation_count: u64,
    pub filesystem_delete_count: u64,
    pub elapsed_micros: u64,
}

impl LiveCorpusCacheStateReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.operation_id.is_empty()
            || self.state != "ready"
            || self.project_id.is_empty()
            || self.workspace_id.is_empty()
            || !matches!(
                self.cache_state.as_str(),
                "cold-build" | "cold-load" | "warm-read" | "released"
            )
            || self.source_workspace_mutation_count != 0
            || self.filesystem_delete_count != 0
        {
            return Err("Live Corpus cache-state receipt is invalid".to_owned());
        }
        if let Some(digest) = self.generation_digest.as_deref()
            && !valid_blake3_digest(digest)
        {
            return Err("Live Corpus cache-state generation digest is invalid".to_owned());
        }
        if let Some(digest) = self.root_digest.as_deref()
            && !valid_hex_digest(digest)
        {
            return Err("Live Corpus cache-state root digest is invalid".to_owned());
        }
        Ok(())
    }
}

fn valid_blake3_digest(value: &str) -> bool {
    value
        .strip_prefix("blake3-256:")
        .is_some_and(valid_hex_digest)
}

fn valid_hex_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Counted Runtime work performed while serving one client operation.
pub struct AspClientRuntimeWorkCounters {
    pub database_read_count: u64,
    pub filesystem_read_count: u64,
    pub provider_process_count: u64,
    pub scheduler_task_count: u64,
    pub socket_operation_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Successful exact-query response with generation identity, result, timing, and work counters.
pub struct AspClientExactQueryResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub language_id: String,
    pub provider_id: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub result: Value,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: AspClientRuntimeWorkCounters,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Typed exact-query failure with selector evidence and no prescribed continuation.
pub struct AspClientExactQueryFailure {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub operation_id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub language_id: String,
    pub provider_id: String,
    pub requested_selector: Option<String>,
    pub resolved_selector: Option<String>,
    pub projection_kind: Option<String>,
    pub phase: String,
    pub reason_kind: String,
    pub generation_digest: Option<String>,
    pub root_digest: Option<String>,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: AspClientRuntimeWorkCounters,
    pub details: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Runtime dispatch request for provider-backed search execution.
pub struct RuntimeProviderSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub language_id: String,
    pub scope: String,
    pub query_plan: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Source-bound request for a provider-native exact projection.
pub struct ProviderNativeExactRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub projection_kind: String,
    pub structural_selector: String,
    pub owner_path: String,
    pub generation_identity_digest: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub source_digest: String,
    pub source_byte_length: usize,
    pub source_encoding: String,
    pub source_bytes_base64: String,
    pub transport: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Provider-native exact projection or typed selector-resolution terminal.
pub struct ProviderNativeExactProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub owner_path: String,
    pub requested_structural_selector: String,
    #[serde(default)]
    pub resolution_state: Option<String>,
    #[serde(default)]
    pub reason_kind: Option<String>,
    #[serde(default)]
    pub active_generation_digest: Option<String>,
    #[serde(default)]
    pub root_digest: Option<String>,
    #[serde(default)]
    pub item_kind: Option<String>,
    #[serde(default)]
    pub item_name: Option<String>,
    #[serde(default)]
    pub candidates: Option<Vec<String>>,
    #[serde(default)]
    pub actual_kinds: Option<Vec<String>>,
    #[serde(default)]
    pub recommended_next: Option<Value>,
    #[serde(default)]
    pub structural_selector: Option<String>,
    #[serde(default)]
    pub projection_mode: Option<String>,
    #[serde(default)]
    pub normalized_parser_facts: Option<Value>,
    #[serde(default)]
    pub projection_text: Option<String>,
    #[serde(default)]
    pub projection_payload: Option<Value>,
    #[serde(default)]
    pub source_content_digest: Option<String>,
    #[serde(default)]
    pub source_byte_start: Option<usize>,
    #[serde(default)]
    pub source_byte_end: Option<usize>,
}

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

impl AspClientSearchRequest {
    pub fn playbook(intent: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            schema_id: CLIENT_SEARCH_REQUEST.to_owned(),
            schema_version: "1".to_owned(),
            intent: intent.into(),
            query: query.into(),
            scope: "workspace".to_owned(),
            coverage: "candidates".to_owned(),
            max_owners: 100,
            deadline_ms: 1_000,
            explain: "compact".to_owned(),
        }
    }

    pub fn validate_schema_identity(&self) -> Result<(), String> {
        check(&self.schema_id, CLIENT_SEARCH_REQUEST, &self.schema_version)?;
        if !matches!(
            self.intent.as_str(),
            "conceptual" | "relationship" | "exact-literal" | "absence-proof"
        ) {
            return Err("ASP client search intent is unsupported".to_owned());
        }
        if self.query.trim().is_empty() {
            return Err("ASP client search query must not be empty".to_owned());
        }
        if self.scope != "workspace"
            && !self
                .scope
                .strip_prefix("owner:")
                .is_some_and(|owner| !owner.trim().is_empty())
        {
            return Err("ASP client search scope is unsupported".to_owned());
        }
        if !matches!(self.coverage.as_str(), "candidates" | "complete")
            || (self.coverage == "complete" && self.intent != "absence-proof")
        {
            return Err("ASP client search coverage is unsupported".to_owned());
        }
        if self.max_owners == 0 || self.max_owners > 100 {
            return Err("ASP client search maxOwners is out of bounds".to_owned());
        }
        if self.deadline_ms == 0 || self.deadline_ms > 5_000 {
            return Err("ASP client search deadlineMs is out of bounds".to_owned());
        }
        if !matches!(self.explain.as_str(), "compact" | "full") {
            return Err("ASP client search explain mode is unsupported".to_owned());
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
        for (name, value) in [
            ("languages", self.languages.as_deref()),
            ("documents", self.documents.as_deref()),
            ("workspace", self.workspace.as_deref()),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(format!("ASP workspace Search {name} must not be empty"));
            }
        }

        let acquisition_count = self.fd.as_ref().map_or(0, Vec::len)
            + self.rg.as_ref().map_or(0, Vec::len)
            + self.tantivy.as_ref().map_or(0, Vec::len)
            + self.syntax.as_ref().map_or(0, Vec::len)
            + self.native_syntax.as_ref().map_or(0, Vec::len);
        let graph_count = self.graph.as_ref().map_or(0, Vec::len);
        if acquisition_count == 0 && graph_count != 0 {
            return Err("ASP workspace Search Graph requires preceding acquisition".to_owned());
        }
        if acquisition_count != 0 && self.languages.is_none() && self.documents.is_none() {
            return Err(
                "ASP workspace Search Playbook execution requires languages or documents"
                    .to_owned(),
            );
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
                AspClientSearchPlaybookClauseAxis::Fd => self.fd.as_ref().map_or(0, Vec::len),
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

        for argv in self
            .fd
            .iter()
            .chain(self.rg.iter())
            .chain(self.tantivy.iter())
            .flatten()
        {
            if argv.is_empty() {
                return Err("ASP workspace Search native argv must not be empty".to_owned());
            }
        }
        if self
            .syntax
            .iter()
            .flatten()
            .any(|block| block.producer.is_empty() || block.argv.is_empty())
        {
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
        check(
            &self.schema_id,
            CLIENT_WORKSPACE_QUERY_PLAYBOOK_REQUEST,
            &self.schema_version,
        )?;
        if self.selectors.is_empty()
            || self
                .selectors
                .iter()
                .any(|selector| !selector.contains("://") || !selector.contains("#item/"))
            || self.selectors.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "workspace Query Playbook selectors must be canonical, unique, and sorted"
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
        if self.languages.is_none() && self.documents.is_none() {
            return Err("workspace syntax Query requires a producer selector".to_owned());
        }
        if self.syntax.is_empty()
            || self.syntax.iter().any(|block| {
                block.producer.trim().is_empty()
                    || block.argv.is_empty()
                    || block.argv.iter().any(|argument| argument.is_empty())
            })
        {
            return Err("workspace syntax Query requires complete native blocks".to_owned());
        }
        if self.projection != "matches" {
            return Err("workspace syntax Query projection must be matches".to_owned());
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
