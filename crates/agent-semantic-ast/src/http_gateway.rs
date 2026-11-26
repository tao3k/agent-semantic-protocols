// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-agnostic HTTP-JSON Forwarding Gateway (`AspForwardingGateway`).
//!
//! Realizes the Two-Plane Leveraged Architecture (RFC 10.36):
//! - **General JSON Data Plane** (`POST /v1/open-edb/inspect-files`): routes
//!   arbitrary workspace source files by extension through [`AstProviderRegistry`]
//!   to live external native compilers (`ast-inspect`) and forwards uniform
//!   `semantic-open-edb.v1` [`WorkspaceOpenEdbIngestBatch`] payloads over HTTP.
//! - **Targeted Schema & DSL Plane** (`GET /v1/providers`,
//!   `POST /v1/providers/register`): forwards each provider's self-reported
//!   [`ProviderCoreManifest`], targeted JSON Schema (constraining native
//!   `languageKind` & `kind`), and optional [`LanguageConformancePack`] DSL
//!   descriptor to Marlin so Marlin can compile contracts via Yuanxu and mix
//!   Special `:gerbil-ascent` relations dynamically without recompiling ASP or Marlin.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use agent_semantic_http_json::{HttpJsonRequest, HttpJsonResponse, serve_http_json};
use agent_semantic_provider_protocol::{LanguageConformancePack, ProviderCoreManifest};
use agent_semantic_search::WorkspaceOpenEdbIngestBatch;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::open_edb_projection::project_nodes_to_open_edb_batch;
use crate::provider_port::AstInspectRequest;
use crate::registry::{AstProviderRegisterReceipt, AstProviderRegistry};
use crate::span_overlay::blake3_source_digest;
use crate::types::{AstEngineError, AstLanguageId, AstWorkspacePath};

/// Schema ID for the ASP provider forwarding catalog response.
pub const PROVIDER_FORWARDING_CATALOG_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-forwarding-catalog";
/// Schema ID for the ASP Open EDB HTTP forwarding response.
pub const OPEN_EDB_FORWARDING_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.open-edb-forwarding-response";

/// Complete provider forwarding envelope delivered over HTTP to Marlin:
/// pairs the provider's self-reported manifest with its targeted JSON Schema
/// and optional Scheme-aligned DSL pack (`kindMappings` + `specialRelations`).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderForwardingEnvelope {
    pub manifest: ProviderCoreManifest,
    pub targeted_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_pack: Option<LanguageConformancePack>,
}

/// Catalog payload returned by `GET /v1/providers`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderForwardingCatalog {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: u64,
    pub registry_digest: String,
    pub providers: Vec<ProviderForwardingEnvelope>,
}

/// Dynamic runtime provider registration frame accepted by `POST /v1/providers/register`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicProviderRegistrationRequest {
    pub manifest: ProviderCoreManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targeted_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_pack: Option<LanguageConformancePack>,
}

/// One workspace file submitted to `POST /v1/open-edb/inspect-files`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedInspectFileInput {
    pub path: String,
    pub source_utf8: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_override: Option<String>,
}

/// Batch file inspection request accepted by `POST /v1/open-edb/inspect-files`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedInspectFilesRequest {
    pub files: Vec<ForwardedInspectFileInput>,
}

/// General JSON Open EDB forwarding response returned by `POST /v1/open-edb/inspect-files`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedOpenEdbResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub inspected_files: usize,
    pub total_entities: usize,
    pub elapsed_micros: u64,
    pub batches: Vec<WorkspaceOpenEdbIngestBatch>,
    pub skipped_paths: Vec<String>,
}

/// Derive a Yuanxu-compatible targeted JSON Schema enforcing a language's exact
/// `languageId`, `kind`, and native `languageKind` surface from its DSL pack.
#[must_use]
pub fn derive_targeted_entity_schema(
    language_id: &AstLanguageId,
    dsl_pack: Option<&LanguageConformancePack>,
) -> Value {
    let mut native_kinds = Vec::new();
    let mut general_kinds = Vec::new();
    if let Some(pack) = dsl_pack {
        for mapping in &pack.kind_mappings {
            if !native_kinds.contains(&mapping.native_language_kind) {
                native_kinds.push(mapping.native_language_kind.clone());
            }
            if !general_kinds.contains(&mapping.general_kind) {
                general_kinds.push(mapping.general_kind.clone());
            }
        }
    }
    let kind_prop = if general_kinds.is_empty() {
        json!({ "type": "string", "minLength": 1 })
    } else {
        json!({ "type": "string", "enum": general_kinds })
    };
    let language_kind_prop = if native_kinds.is_empty() {
        json!({ "type": "string", "minLength": 1 })
    } else {
        json!({ "type": "string", "enum": native_kinds })
    };
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["languageId", "kind", "languageKind", "name", "line"],
        "properties": {
            "languageId": { "type": "string", "const": language_id.as_str() },
            "kind": kind_prop,
            "languageKind": language_kind_prop,
            "name": { "type": "string", "minLength": 1 },
            "line": { "type": "integer", "minimum": 1 }
        }
    })
}

/// Language-agnostic HTTP-JSON forwarding gateway pairing an [`AstProviderRegistry`]
/// with dynamically forwarded provider manifests, targeted schemas, and DSL packs.
#[derive(Default)]
pub struct AspForwardingGateway {
    registry: RwLock<AstProviderRegistry>,
    envelopes: RwLock<BTreeMap<AstLanguageId, ProviderForwardingEnvelope>>,
}

impl AspForwardingGateway {
    /// Construct an empty [`AspForwardingGateway`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one live HTTP-JSON external language provider along with its
    /// optional DSL pack and targeted JSON Schema.
    pub fn register_http_provider(
        &self,
        manifest: ProviderCoreManifest,
        endpoint: String,
        dsl_pack: Option<LanguageConformancePack>,
        targeted_schema: Option<Value>,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        let language_id = AstLanguageId::new(manifest.language_id.as_str())?;
        let schema = targeted_schema
            .unwrap_or_else(|| derive_targeted_entity_schema(&language_id, dsl_pack.as_ref()));
        let receipt = {
            let mut reg = self
                .registry
                .write()
                .map_err(|_| AstEngineError::ProviderError("registry lock poisoned".to_owned()))?;
            reg.register_http_from_manifest(&manifest, endpoint)?
        };
        let mut envs = self
            .envelopes
            .write()
            .map_err(|_| AstEngineError::ProviderError("envelopes lock poisoned".to_owned()))?;
        envs.insert(
            language_id,
            ProviderForwardingEnvelope {
                manifest,
                targeted_schema: schema,
                dsl_pack,
            },
        );
        Ok(receipt)
    }

    /// Register a schema/DSL-only provider envelope (or update an existing
    /// language's targeted schema & DSL without restarting ASP).
    pub fn register_descriptor_only(
        &self,
        manifest: ProviderCoreManifest,
        dsl_pack: Option<LanguageConformancePack>,
        targeted_schema: Option<Value>,
    ) -> Result<ProviderForwardingEnvelope, AstEngineError> {
        let language_id = AstLanguageId::new(manifest.language_id.as_str())?;
        let schema = targeted_schema
            .unwrap_or_else(|| derive_targeted_entity_schema(&language_id, dsl_pack.as_ref()));
        let envelope = ProviderForwardingEnvelope {
            manifest,
            targeted_schema: schema,
            dsl_pack,
        };
        let mut envs = self
            .envelopes
            .write()
            .map_err(|_| AstEngineError::ProviderError("envelopes lock poisoned".to_owned()))?;
        envs.insert(language_id, envelope.clone());
        Ok(envelope)
    }

    /// Snapshot the current provider forwarding catalog for `GET /v1/providers`.
    pub fn catalog(&self) -> Result<ProviderForwardingCatalog, AstEngineError> {
        let (generation, digest) = {
            let reg = self
                .registry
                .read()
                .map_err(|_| AstEngineError::ProviderError("registry lock poisoned".to_owned()))?;
            (reg.generation(), reg.registry_digest())
        };
        let providers = self
            .envelopes
            .read()
            .map_err(|_| AstEngineError::ProviderError("envelopes lock poisoned".to_owned()))?
            .values()
            .cloned()
            .collect();
        Ok(ProviderForwardingCatalog {
            schema_id: PROVIDER_FORWARDING_CATALOG_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            generation,
            registry_digest: digest.as_str().to_owned(),
            providers,
        })
    }

    /// Inspect a batch of workspace files by routing each through its registered
    /// native compiler provider and projecting uniform Open EDB General JSON batches.
    pub async fn inspect_files_to_open_edb(
        self: &Arc<Self>,
        request: ForwardedInspectFilesRequest,
    ) -> Result<ForwardedOpenEdbResponse, AstEngineError> {
        let started = Instant::now();
        let mut batches = Vec::with_capacity(request.files.len());
        let mut skipped_paths = Vec::new();
        let mut total_entities = 0usize;

        for file in request.files {
            let path = AstWorkspacePath::new(&file.path)?;
            let override_lang = match file.language_override.as_deref() {
                Some(raw) => Some(AstLanguageId::new(raw)?),
                None => None,
            };
            let resolved = {
                let reg = self.registry.read().map_err(|_| {
                    AstEngineError::ProviderError("registry lock poisoned".to_owned())
                })?;
                reg.resolve(&path, override_lang.as_ref())
            };
            let Ok(provider) = resolved else {
                skipped_paths.push(file.path);
                continue;
            };
            let source_hash = blake3_source_digest(file.source_utf8.as_bytes());
            let inspect_req = AstInspectRequest {
                path: path.clone(),
                source_utf8: file.source_utf8,
                source_hash,
            };
            let port = Arc::clone(&provider.port);
            let inspected = tokio::task::spawn_blocking(move || port.inspect(&inspect_req))
                .await
                .map_err(|err| AstEngineError::ProviderError(format!("inspect join: {err}")))??;
            let batch = project_nodes_to_open_edb_batch(
                &path,
                &provider.descriptor.language_id,
                &provider.descriptor.provider_id,
                &inspected.nodes,
            );
            total_entities += batch.entities.len();
            batches.push(batch);
        }

        let elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        Ok(ForwardedOpenEdbResponse {
            schema_id: OPEN_EDB_FORWARDING_RESPONSE_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            inspected_files: batches.len(),
            total_entities,
            elapsed_micros,
            batches,
            skipped_paths,
        })
    }

    /// Serve this gateway over loopback HTTP/1.1 until `/shutdown` or `shutdown_rx`.
    pub async fn serve(
        self: Arc<Self>,
        listener: TcpListener,
        shutdown_tx: watch::Sender<bool>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), String> {
        let gateway = Arc::clone(&self);
        serve_http_json(listener, shutdown_rx, move |req: HttpJsonRequest| {
            let gw = Arc::clone(&gateway);
            let shutdown = shutdown_tx.clone();
            async move { handle_gateway_request(gw, shutdown, req).await }
        })
        .await
    }
}

async fn handle_gateway_request(
    gateway: Arc<AspForwardingGateway>,
    shutdown_tx: watch::Sender<bool>,
    req: HttpJsonRequest,
) -> Result<HttpJsonResponse, String> {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/health") => {
            let cat = gateway.catalog().map_err(|e| e.to_string())?;
            HttpJsonResponse::json(
                200,
                &json!({
                    "status": "ready",
                    "providerCount": cat.providers.len(),
                    "generation": cat.generation,
                    "registryDigest": cat.registry_digest
                }),
            )
        }
        ("GET", "/v1/providers") => {
            let cat = gateway.catalog().map_err(|e| e.to_string())?;
            let val = serde_json::to_value(&cat).map_err(|e| e.to_string())?;
            HttpJsonResponse::json(200, &val)
        }
        ("POST", "/v1/providers/register") => {
            let payload: DynamicProviderRegistrationRequest = serde_json::from_slice(&req.body)
                .map_err(|e| format!("decode register request: {e}"))?;
            let response = match payload.endpoint {
                Some(ep) => {
                    let receipt = gateway
                        .register_http_provider(
                            payload.manifest,
                            ep,
                            payload.dsl_pack,
                            payload.targeted_schema,
                        )
                        .map_err(|e| e.to_string())?;
                    json!({
                        "status": "registered",
                        "mode": "live-http",
                        "generation": receipt.generation,
                        "registeredCount": receipt.registered_count
                    })
                }
                None => {
                    let env = gateway
                        .register_descriptor_only(
                            payload.manifest,
                            payload.dsl_pack,
                            payload.targeted_schema,
                        )
                        .map_err(|e| e.to_string())?;
                    json!({
                        "status": "registered",
                        "mode": "descriptor-only",
                        "languageId": env.manifest.language_id.as_str(),
                        "providerId": env.manifest.provider_id.as_str()
                    })
                }
            };
            HttpJsonResponse::json(200, &response)
        }
        ("POST", "/v1/open-edb/inspect-files") => {
            let payload: ForwardedInspectFilesRequest = serde_json::from_slice(&req.body)
                .map_err(|e| format!("decode inspect-files request: {e}"))?;
            let result = gateway
                .inspect_files_to_open_edb(payload)
                .await
                .map_err(|e| e.to_string())?;
            let val = serde_json::to_value(&result).map_err(|e| e.to_string())?;
            HttpJsonResponse::json(200, &val)
        }
        ("POST", "/shutdown") => {
            let _ = shutdown_tx.send(true);
            HttpJsonResponse::json(200, &json!({ "status": "draining" }))
        }
        _ => HttpJsonResponse::json(
            404,
            &json!({ "error": format!("unknown route: {} {}", req.method, req.path) }),
        ),
    }
}
