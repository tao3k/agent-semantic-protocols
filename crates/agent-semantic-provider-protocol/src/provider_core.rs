// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Converged minimal AI-Native Language Server Provider Protocol (RFC 10.36).
//!
//! Plane 1 Single Source of Truth (Rust + `schemars` Draft 2020-12):
//! - External native language providers (`asp-rust`, `asp-python`, `asp-typescript`,
//!   `AspJulia.jl`, `asp-gerbil-scheme`, ...) implement ONLY the minimal compiler
//!   wire spine (`ast-inspect`, `ast-verify`, `semantic-open-edb`, plus optional
//!   `project-resolution`) and self-report their native [`ProviderSyntaxSurface`]
//!   (comment syntax, delimiter pairs, statement terminator, string quotes).
//! - Plane 2 high-order Language Ontology, `:gerbil-ascent` search relations, and
//!   Coding Style policies live exclusively in Gerbil Scheme (`defasp-language`)
//!   and project compact [`LanguageConformancePack`] data packs consumed by
//!   ASP's black-box provider conformance harness.
//! - Schema version metadata lives strictly inside the schema/document payload
//!   (`schemaVersion`), never encoded in filenames.

use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Schema ID for the converged provider core manifest.
pub const PROVIDER_CORE_MANIFEST_SCHEMA_ID: &str = "agent.semantic-protocols.provider-core-manifest";
/// Current schema version carried inside the provider core manifest payload.
pub const PROVIDER_CORE_MANIFEST_SCHEMA_VERSION: &str = "1";
/// Schema ID for `ast-inspect` wire messages.
pub const AST_INSPECT_SCHEMA_ID: &str = "agent.semantic-protocols.ast-inspect";
/// Schema ID for `ast-verify` wire messages.
pub const AST_VERIFY_SCHEMA_ID: &str = "agent.semantic-protocols.ast-verify";
/// Schema ID for Scheme-projected language conformance packs.
pub const LANGUAGE_CONFORMANCE_PACK_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-conformance-pack";
/// Current schema version carried inside AST operation & conformance pack payloads.
pub const PROVIDER_CORE_SCHEMA_VERSION: &str = "1";

/// Canonical filenames of the converged Provider Core Schema Bundle (version-free filenames).
pub const PROVIDER_CORE_SCHEMAS: &[&str] = &[
    "provider-core-manifest.schema.json",
    "provider-runtime-request-frame.schema.json",
    "provider-runtime-response-frame.schema.json",
    "semantic-open-edb.v1.schema.json",
    "ast-inspect.schema.json",
    "ast-verify.schema.json",
    "language-conformance-pack.schema.json",
];

/// Canonical language identifier newtype (e.g. `rust`, `python`, `julia`, `gerbil-scheme`).
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderLanguageId(String);

impl ProviderLanguageId {
    /// Validate and construct a lowercase canonical language identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let value = raw.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(format!("invalid provider language id: {value}"));
        }
        Ok(Self(value))
    }

    /// Borrow the canonical language identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Canonical provider identifier newtype (e.g. `asp-rust`, `asp-julia`, `asp-gerbil-scheme`).
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderPackageId(String);

impl ProviderPackageId {
    /// Validate and construct a lowercase canonical provider identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let value = raw.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(format!("invalid provider package id: {value}"));
        }
        Ok(Self(value))
    }

    /// Borrow the canonical provider identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-reported native lexical syntax surface (owned by the language provider,
/// never duplicated in Scheme ontology definitions).
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSyntaxSurface {
    /// Line comment opener reported by the native parser (e.g. `//`, `#`, `;`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_comment: Option<String>,
    /// Block comment `[open, close]` pair reported by the native parser (e.g. `["/*", "*/"]`, `["#|", "|#"]`, `["#=", "=#"]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_comment: Option<[String; 2]>,
    /// Balanced delimiter `[open, close]` pairs native to this language.
    #[serde(default)]
    pub delimiter_pairs: Vec<[String; 2]>,
    /// Optional statement terminator token (e.g. `;` for Rust/TypeScript, absent for Python/Julia/Scheme).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_terminator: Option<String>,
    /// Native string literal quote delimiters (e.g. `["\""]`, `["\"", "'"]`).
    #[serde(default)]
    pub string_delimiters: Vec<String>,
}

/// Minimal HTTP-JSON transport descriptor for a provider server.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTransport {
    /// Wire transport kind (`http-json`).
    pub transport: String,
    /// Subcommand arguments launching the resident provider server (e.g. `["serve"]`).
    pub command: Vec<String>,
    /// Health and handshake endpoint path (default `/health`).
    pub health_path: String,
    /// Request frame dispatch endpoint path (default `/v1/provider-runtime`).
    pub request_path: String,
    /// Graceful drain endpoint path (default `/shutdown`).
    pub shutdown_path: String,
}

/// Minimal capability flags published by a native language provider.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderCapabilities {
    /// Provider implements `ast-inspect` returning authoritative UTF-8 byte spans.
    pub ast_inspect: bool,
    /// Provider implements `ast-verify` batched compiler soundness oracle.
    pub ast_verify: bool,
    /// Provider emits Open EDB facts directly or via `ast-inspect`.
    pub open_edb: bool,
    /// Provider optionally implements `project-resolution` workspace membership.
    #[serde(default)]
    pub project_resolution: bool,
}

/// Converged Provider Core Manifest (`provider-core-manifest.schema.json`).
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderCoreManifest {
    /// Canonical schema identifier (`agent.semantic-protocols.provider-core-manifest`).
    pub schema_id: String,
    /// Schema version carried inside the document payload (not in the filename).
    pub schema_version: String,
    /// Canonical language identifier served by this provider.
    pub language_id: ProviderLanguageId,
    /// Canonical provider package identifier.
    pub provider_id: ProviderPackageId,
    /// Human-readable display name.
    pub display_name: String,
    /// Lowercase leading-dot source extensions owned by this provider (e.g. `[".rs"]`).
    pub extensions: Vec<String>,
    /// Project root marker files (e.g. `["Cargo.toml"]`, `["Project.toml"]`).
    #[serde(default)]
    pub project_markers: Vec<String>,
    /// Native lexical syntax surface reported by the provider itself.
    pub syntax_surface: ProviderSyntaxSurface,
    /// Resident HTTP-JSON server launch and route descriptor.
    pub transport: ProviderTransport,
    /// Admitted compiler primitives.
    pub capabilities: ProviderCapabilities,
}

/// Half-open UTF-8 byte span `[start, end)` referencing immutable source bytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceByteSpan {
    pub start: usize,
    pub end: usize,
}

/// Open key/value attribute facet attached to an inspected AST node.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstNodeAttribute {
    pub key: String,
    pub value: String,
}

/// Authoritative native AST node span emitted by `ast-inspect`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstInspectNode {
    pub locator: String,
    pub kind: String,
    pub language_kind: String,
    pub name: String,
    pub line: usize,
    pub outer_span: SourceByteSpan,
    pub inner_span: SourceByteSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_span: Option<SourceByteSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub attrs: Vec<AstNodeAttribute>,
}

/// Request payload for the `ast-inspect` provider primitive.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstInspectRequest {
    pub path: String,
    pub source_utf8: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
}

/// Response payload for the `ast-inspect` provider primitive.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstInspectResponse {
    pub accepted: bool,
    pub nodes: Vec<AstInspectNode>,
    #[serde(default)]
    pub diagnostics: Vec<AstVerifyDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_surface: Option<ProviderSyntaxSurface>,
}

/// Combined wire contract document for `ast-inspect.schema.json`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstInspectContract {
    pub schema_version: String,
    pub request: AstInspectRequest,
    pub response: AstInspectResponse,
}

/// One candidate buffer submitted to the `ast-verify` compiler soundness oracle.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyCandidate {
    pub candidate_id: String,
    pub source_utf8: String,
}

/// Request payload for the `ast-verify` provider primitive.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyRequest {
    pub path: String,
    pub candidates: Vec<AstVerifyCandidate>,
}

/// Compiler parse/syntax diagnostic returned by `ast-inspect` or `ast-verify`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyDiagnostic {
    pub byte_start: usize,
    pub byte_end: usize,
    pub message: String,
    #[serde(default)]
    pub expected_tokens: Vec<String>,
}

/// Outcome for one candidate evaluated by `ast-verify`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyOutcome {
    pub candidate_id: String,
    pub accepted: bool,
    #[serde(default)]
    pub diagnostics: Vec<AstVerifyDiagnostic>,
}

/// Response payload for the `ast-verify` provider primitive.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyResponse {
    pub results: Vec<AstVerifyOutcome>,
}

/// Combined wire contract document for `ast-verify.schema.json`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstVerifyContract {
    pub schema_version: String,
    pub request: AstVerifyRequest,
    pub response: AstVerifyResponse,
}

/// Mapping from a language's native AST `language_kind` to ASP's General Ontology `kind`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OntologyKindMapping {
    /// Native compiler AST node kind (e.g. `fn_item`, `FunctionDef`, `struct`, `poo_def`).
    pub native_language_kind: String,
    /// Universal General Ontology kind (e.g. `function`, `struct`, `class`, `macro`, `item`).
    pub general_kind: String,
    /// Concise semantic description inherited from Scheme's `defasp-language`.
    pub description: String,
}

/// Expected AST entity assertion inside a Scheme-projected conformance probe.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedConformanceNode {
    pub kind: String,
    pub language_kind: String,
    pub name: String,
}

/// Minimal golden conformance probe verifying a native provider's `ast-inspect` and `ast-verify`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageConformanceProbe {
    pub probe_id: String,
    pub path: String,
    pub source: String,
    pub expect_valid: bool,
    #[serde(default)]
    pub expected_nodes: Vec<ExpectedConformanceNode>,
}

/// Compact pure-data Language Conformance Pack projected from Marlin's Gerbil Scheme
/// `defasp-language` POO General Ontology and consumed by ASP's black-box provider harness.
///
/// Contains ZERO low-level lexical rules (comment/pair syntax is reported by the native
/// provider itself via [`ProviderSyntaxSurface`]).
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageConformancePack {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: ProviderLanguageId,
    pub provider_language_id: ProviderLanguageId,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub project_markers: Vec<String>,
    pub kind_mappings: Vec<OntologyKindMapping>,
    #[serde(default)]
    pub special_relations: Vec<String>,
    pub probes: Vec<LanguageConformanceProbe>,
}

fn stamp_canonical_schema(mut value: Value, filename: &str, title: &str, version: &str) -> Value {
    if let Value::Object(ref mut obj) = value {
        let mut ordered = Map::new();
        if let Some(schema_uri) = obj.remove("$schema") {
            ordered.insert("$schema".to_owned(), schema_uri);
        } else {
            ordered.insert(
                "$schema".to_owned(),
                Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
            );
        }
        ordered.insert(
            "$id".to_owned(),
            Value::String(format!(
                "https://agent-semantic-protocols.local/schemas/{filename}"
            )),
        );
        ordered.insert("title".to_owned(), Value::String(title.to_owned()));
        ordered.insert("x-schema-version".to_owned(), Value::String(version.to_owned()));
        obj.remove("title");
        obj.remove("$id");
        for (k, v) in std::mem::take(obj) {
            ordered.insert(k, v);
        }
        *obj = ordered;
    }
    value
}

/// Generate all Rust-owned Provider Core canonical Draft 2020-12 JSON Schema documents
/// directly from typed protocol structs via `schemars` (zero hand-written JSON,
/// version metadata carried inside the schema rather than in filenames).
pub fn canonical_provider_core_schemas() -> Result<BTreeMap<&'static str, Value>, String> {
    let entries: [(&'static str, &'static str, &'static str, Value); 4] = [
        (
            "provider-core-manifest.schema.json",
            "ASP Provider Core Manifest",
            PROVIDER_CORE_MANIFEST_SCHEMA_VERSION,
            serde_json::to_value(schema_for!(ProviderCoreManifest))
                .map_err(|e| format!("encode provider-core-manifest schema: {e}"))?,
        ),
        (
            "ast-inspect.schema.json",
            "ASP Provider AST Inspect Contract",
            PROVIDER_CORE_SCHEMA_VERSION,
            serde_json::to_value(schema_for!(AstInspectContract))
                .map_err(|e| format!("encode ast-inspect schema: {e}"))?,
        ),
        (
            "ast-verify.schema.json",
            "ASP Provider AST Verify Contract",
            PROVIDER_CORE_SCHEMA_VERSION,
            serde_json::to_value(schema_for!(AstVerifyContract))
                .map_err(|e| format!("encode ast-verify schema: {e}"))?,
        ),
        (
            "language-conformance-pack.schema.json",
            "ASP Language Conformance Pack",
            PROVIDER_CORE_SCHEMA_VERSION,
            serde_json::to_value(schema_for!(LanguageConformancePack))
                .map_err(|e| format!("encode language-conformance-pack schema: {e}"))?,
        ),
    ];
    Ok(entries
        .into_iter()
        .map(|(name, title, version, raw)| (name, stamp_canonical_schema(raw, name, title, version)))
        .collect())
}
