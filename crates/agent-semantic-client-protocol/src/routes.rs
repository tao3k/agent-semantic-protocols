// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed provider route request and response bindings for the ASP Client Protocol.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

pub(super) const SEARCH_REQUEST: &str = "agent.semantic-protocols.runtime-provider-search-request";
pub(super) const CLIENT_WORKSPACE_SEARCH_PLAYBOOK_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-search-playbook-request";
pub(super) const CLIENT_WORKSPACE_QUERY_PLAYBOOK_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-query-playbook-request";
pub(super) const CLIENT_WORKSPACE_SYNTAX_QUERY_REQUEST: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-query-request";
pub(super) const CLIENT_SOURCE_INDEX_LOOKUP_REQUEST: &str =
    "agent.semantic-protocols.asp-client-source-index-lookup-request";
pub(super) const CLIENT_EXACT_QUERY_REQUEST: &str =
    "agent.semantic-protocols.asp-client-exact-query-request";
pub(super) const CLIENT_EXACT_QUERY_RESPONSE: &str =
    "agent.semantic-protocols.asp-client-exact-query-response";
pub(super) const CLIENT_EXACT_QUERY_FAILURE: &str =
    "agent.semantic-protocols.asp-client-exact-query-failure";
pub(super) const CLIENT_GRAPHS_TIMELINE_REQUEST: &str =
    "agent.semantic-protocols.asp-client-graphs-timeline-request";
/// Schema identity for live-corpus cache-state requests.
pub const LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.live-corpus-cache-state-request";
/// Schema identity for live-corpus cache-state receipts.
pub const LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.live-corpus-cache-state-receipt";
pub(super) const EXACT_REQUEST: &str = "agent.semantic-protocols.provider-native-exact-request";
pub(super) const EXACT_RESPONSE: &str = "agent.semantic-protocols.provider-native-exact-projection";

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
/// before Runtime dispatch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientWorkspaceSearchPlaybookRequest {
    pub schema_id: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
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

pub const RUNTIME_RESIDENT_REQUEST_PLANE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-resident-request-plane-receipt";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeResidentRequestOperation {
    Search,
    Query,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeResidentRequestTemperature {
    Cold,
    Warm,
}

/// Runtime-emitted evidence for one admitted resident Search/Query request.
/// Fields intentionally mirror the V1 schema and avoid `Default`, so a caller
/// must construct the complete zero-external-work claim explicitly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeResidentRequestPlaneReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: RuntimeResidentRequestOperation,
    pub request_temperature: RuntimeResidentRequestTemperature,
    pub state: String,
    pub generation_digest: Option<String>,
    pub elapsed_micros: u64,
    pub generation_lookup_count: u64,
    pub generation_wait_count: u64,
    pub generation_build_count: u64,
    pub filesystem_read_count: u64,
    pub database_read_count: u64,
    pub provider_process_count: u64,
    pub parser_invocation_count: u64,
    pub secondary_runtime_rpc_count: u64,
    pub socket_discovery_count: u64,
    pub terminal_wait_count: u64,
}

impl RuntimeResidentRequestPlaneReceipt {
    #[must_use]
    pub fn ready(
        operation: RuntimeResidentRequestOperation,
        request_temperature: RuntimeResidentRequestTemperature,
        generation_digest: String,
        elapsed_micros: u64,
    ) -> Self {
        Self::new(
            operation,
            request_temperature,
            "ready",
            Some(generation_digest),
            elapsed_micros,
        )
    }

    #[must_use]
    pub fn query_not_ready(
        operation: RuntimeResidentRequestOperation,
        request_temperature: RuntimeResidentRequestTemperature,
        elapsed_micros: u64,
    ) -> Self {
        Self::new(
            operation,
            request_temperature,
            "query-not-ready",
            None,
            elapsed_micros,
        )
    }

    fn new(
        operation: RuntimeResidentRequestOperation,
        request_temperature: RuntimeResidentRequestTemperature,
        state: &str,
        generation_digest: Option<String>,
        elapsed_micros: u64,
    ) -> Self {
        Self {
            schema_id: RUNTIME_RESIDENT_REQUEST_PLANE_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            operation,
            request_temperature,
            state: state.to_owned(),
            generation_digest,
            elapsed_micros,
            generation_lookup_count: 1,
            generation_wait_count: 0,
            generation_build_count: 0,
            filesystem_read_count: 0,
            database_read_count: 0,
            provider_process_count: 0,
            parser_invocation_count: 0,
            secondary_runtime_rpc_count: 0,
            socket_discovery_count: 0,
            terminal_wait_count: 0,
        }
    }
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
