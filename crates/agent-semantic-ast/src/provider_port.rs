// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure Native Language Provider AST Port (`AstProviderPort`), Resident Actor
//! adapter (`ResidentActorAstProvider`), and loopback HTTP-JSON client
//! (`HttpJsonAstProviderClient`) speaking ASP's `POST /v1/provider-runtime`
//! (`ast-inspect` and `ast-verify`).

use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_http_json::post_json;
use agent_semantic_provider_protocol::ProviderSyntaxSurface;
use agent_semantic_provider_transport::{
    ProviderRuntimeActorClient, ProviderRuntimeRequestFrame, ProviderRuntimeResponseFrame,
    ProviderRuntimeResponseOutcome,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::tokio_runtime::run_provider_io;
use crate::types::{
    AstEngineError, AstLanguageId, AstNodeSpanRecord, AstProviderCapabilities, AstProviderId,
    AstSemanticEdgeRecord, AstSyntaxDiagnostic, AstWorkspacePath, Blake3SourceDigest,
    LexicalDelimiterProfile,
};

/// `POST /v1/provider-runtime` operation returning authoritative native AST spans.
pub const AST_INSPECT_OPERATION: &str = "ast-inspect";
/// `POST /v1/provider-runtime` operation running batched native parser verification.
pub const AST_VERIFY_OPERATION: &str = "ast-verify";

const PROVIDER_RUNTIME_PATH: &str = "/v1/provider-runtime";

/// Typed configuration constructing [`ResidentActorAstProvider`] or [`HttpJsonAstProviderClient`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AstProviderClientConfig {
    pub provider_id: AstProviderId,
    pub language_id: AstLanguageId,
    pub lexical_profile: LexicalDelimiterProfile,
    pub capabilities: AstProviderCapabilities,
}

/// Wire request for native AST inspection (`ast-inspect`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstInspectRequest {
    pub path: AstWorkspacePath,
    pub source_utf8: String,
    pub source_hash: Blake3SourceDigest,
}

/// Wire response for native AST inspection (`ast-inspect`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstInspectResponse {
    pub accepted: bool,
    pub nodes: Vec<AstNodeSpanRecord>,
    #[serde(default)]
    pub edges: Vec<AstSemanticEdgeRecord>,
    #[serde(default)]
    pub diagnostics: Vec<AstSyntaxDiagnostic>,
    /// Native lexical syntax surface reported directly by the language provider
    /// (comment openers, delimiter pairs, statement terminator, string quotes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_surface: Option<ProviderSyntaxSurface>,
}

/// One candidate buffer submitted inside a batched [`AstVerifyRequest`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstVerifyCandidate {
    pub candidate_id: String,
    pub source_utf8: String,
}

/// Wire request for batched native compiler soundness verification (`ast-verify`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstVerifyRequest {
    pub path: AstWorkspacePath,
    pub candidates: Vec<AstVerifyCandidate>,
}

/// Native parser verification outcome for a single candidate buffer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstVerifyCandidateResult {
    pub candidate_id: String,
    pub accepted: bool,
    #[serde(default)]
    pub diagnostics: Vec<AstSyntaxDiagnostic>,
}

/// Wire response for batched native compiler soundness verification (`ast-verify`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstVerifyResponse {
    pub results: Vec<AstVerifyCandidateResult>,
}

/// Authoritative native language server boundary consumed by [`crate::AstProviderRegistry`].
pub trait AstProviderPort: Send + Sync {
    /// Canonical provider identifier.
    fn provider_id(&self) -> &AstProviderId;

    /// Canonical language identifier owned by this provider.
    fn language_id(&self) -> &AstLanguageId;

    /// Parameterized lexical delimiter profile for Track-B bounded calibration.
    fn lexical_profile(&self) -> LexicalDelimiterProfile;

    /// Inspect source bytes and return authoritative AST node spans & diagnostics.
    fn inspect(&self, request: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError>;

    /// Verify one or more candidate source buffers against the authoritative native parser.
    fn verify(&self, request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError>;
}

/// Adapter binding ASP's supervised [`ProviderRuntimeActorClient`] directly into
/// [`crate::AstProviderRegistry`].
pub struct ResidentActorAstProvider {
    client: ProviderRuntimeActorClient,
    config: AstProviderClientConfig,
}

impl ResidentActorAstProvider {
    /// Wrap an already-supervised ASP resident provider actor client.
    #[must_use]
    pub fn new(client: ProviderRuntimeActorClient, config: AstProviderClientConfig) -> Self {
        Self { client, config }
    }

    fn call_actor<Req: Serialize, Res: for<'de> Deserialize<'de>>(
        &self,
        operation: &str,
        payload: &Req,
    ) -> Result<Res, AstEngineError> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|err| AstEngineError::ProviderError(err.to_string()))?;
        let client = self.client.clone();
        let op = operation.to_owned();
        let response_bytes = run_provider_io(move || async move {
            client
                .request(op, Bytes::from(payload_bytes))
                .await
                .map_err(AstEngineError::ProviderError)
        })?;
        serde_json::from_slice(&response_bytes)
            .map_err(|err| AstEngineError::ProviderError(err.to_string()))
    }
}

impl AstProviderPort for ResidentActorAstProvider {
    fn provider_id(&self) -> &AstProviderId {
        &self.config.provider_id
    }

    fn language_id(&self) -> &AstLanguageId {
        &self.config.language_id
    }

    fn lexical_profile(&self) -> LexicalDelimiterProfile {
        self.config.lexical_profile.clone()
    }

    fn inspect(&self, request: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
        self.call_actor(AST_INSPECT_OPERATION, request)
    }

    fn verify(&self, request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
        self.call_actor(AST_VERIFY_OPERATION, request)
    }
}

/// Loopback HTTP-JSON client targeting an external ASP language provider's
/// `POST /v1/provider-runtime` server (`asp-rust`, `asp-typescript`,
/// `asp-python`, `AspJulia.jl`, `asp-gerbil-scheme`).
#[derive(Debug)]
pub struct HttpJsonAstProviderClient {
    base_url: String,
    config: AstProviderClientConfig,
    next_request_id: AtomicU64,
}

impl HttpJsonAstProviderClient {
    /// Construct a typed HTTP-JSON AST client targeting `base_url` (e.g. `http://127.0.0.1:43123`).
    #[must_use]
    pub fn new(base_url: impl Into<String>, config: AstProviderClientConfig) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            config,
            next_request_id: AtomicU64::new(1),
        }
    }

    /// Return the configured loopback base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn call_operation<Req: Serialize, Res: for<'de> Deserialize<'de>>(
        &self,
        operation: &str,
        payload: &Req,
    ) -> Result<Res, AstEngineError> {
        let seq = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|err| AstEngineError::ProviderError(err.to_string()))?;
        let frame =
            ProviderRuntimeRequestFrame::new(format!("ast-{seq}"), operation, payload_bytes)
                .map_err(AstEngineError::ProviderError)?;
        let body = serde_json::to_vec(&frame)
            .map_err(|err| AstEngineError::ProviderError(err.to_string()))?;
        let base_url = self.base_url.clone();
        let response = run_provider_io(move || async move {
            post_json(&base_url, PROVIDER_RUNTIME_PATH, Bytes::from(body))
                .await
                .map_err(AstEngineError::ProviderError)
        })?;
        decode_provider_runtime_response(operation, response.status, &response.body)
    }
}

impl AstProviderPort for HttpJsonAstProviderClient {
    fn provider_id(&self) -> &AstProviderId {
        &self.config.provider_id
    }

    fn language_id(&self) -> &AstLanguageId {
        &self.config.language_id
    }

    fn lexical_profile(&self) -> LexicalDelimiterProfile {
        self.config.lexical_profile.clone()
    }

    fn inspect(&self, request: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
        self.call_operation(AST_INSPECT_OPERATION, request)
    }

    fn verify(&self, request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
        self.call_operation(AST_VERIFY_OPERATION, request)
    }
}

fn decode_provider_runtime_response<Res: for<'de> Deserialize<'de>>(
    operation: &str,
    status: u16,
    body: &[u8],
) -> Result<Res, AstEngineError> {
    if status != 200 {
        return Err(AstEngineError::ProviderError(format!(
            "provider HTTP {status} on {operation}"
        )));
    }
    let decoded: ProviderRuntimeResponseFrame = serde_json::from_slice(body)
        .map_err(|err| AstEngineError::ProviderError(err.to_string()))?;
    match decoded.outcome {
        ProviderRuntimeResponseOutcome::Ready => {
            let value = decoded.payload.ok_or_else(|| {
                AstEngineError::ProviderError("missing payload in ready frame".to_owned())
            })?;
            serde_json::from_value(value)
                .map_err(|err| AstEngineError::ProviderError(err.to_string()))
        }
        ProviderRuntimeResponseOutcome::Error => {
            Err(AstEngineError::ProviderError(decoded.error.unwrap_or_else(
                || "provider returned error outcome".to_owned(),
            )))
        }
    }
}
