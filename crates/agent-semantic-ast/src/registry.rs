// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! AI-Native Language Server AST Provider Register Engine (`AstProviderRegistry`).
//!
//! Bridges ASP's canonical [`ProviderRegistrationDocument`] (`agent-semantic-provider-protocol`)
//! and supervised transport bindings (`ResidentActorAstProvider`,
//! `HttpJsonAstProviderClient`, or embedded host ports) into a single
//! language-agnostic routing table keyed by [`AstProviderId`], [`AstLanguageId`],
//! and normalized file extension.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_semantic_provider_protocol::{
    PROVIDER_CORE_MANIFEST_SCHEMA_ID, ProviderCoreManifest, ProviderRegistrationDocument,
};
use serde::{Deserialize, Serialize};

use crate::provider_port::{AstProviderClientConfig, AstProviderPort, HttpJsonAstProviderClient};
use crate::span_overlay::blake3_source_digest;
use crate::types::{
    AstEngineError, AstLanguageId, AstProviderCapabilities, AstProviderId, AstWorkspacePath,
    Blake3SourceDigest, LexicalDelimiterProfile,
};

/// Transport mechanism connecting [`AstProviderRegistry`] to a native provider.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AstProviderTransportKind {
    ResidentActor,
    HttpJsonLoopback,
    EmbeddedPort,
}

/// Binding metadata paired with a [`ProviderRegistrationDocument`] during registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AstProviderDocumentBinding {
    pub lexical_profile: LexicalDelimiterProfile,
    pub capabilities: AstProviderCapabilities,
    pub transport_kind: AstProviderTransportKind,
    pub endpoint: Option<String>,
}

/// Immutable registration descriptor published for one AST language provider.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstProviderDescriptor {
    pub provider_id: AstProviderId,
    pub language_id: AstLanguageId,
    pub source_extensions: Vec<String>,
    pub lexical_profile: LexicalDelimiterProfile,
    pub capabilities: AstProviderCapabilities,
    pub transport_kind: AstProviderTransportKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Immutable snapshot receipt returned after mutating [`AstProviderRegistry`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstProviderRegisterReceipt {
    pub generation: u64,
    pub registry_digest: Blake3SourceDigest,
    pub registered_count: usize,
}

/// Resolved provider entry pairing its registration descriptor with its live port.
#[derive(Clone)]
pub struct RegisteredAstProvider {
    pub descriptor: AstProviderDescriptor,
    pub port: Arc<dyn AstProviderPort>,
}

/// Language-agnostic AI-Native Language Server AST Provider Register Engine.
#[derive(Default)]
pub struct AstProviderRegistry {
    generation: u64,
    providers: BTreeMap<AstProviderId, RegisteredAstProvider>,
    by_language: BTreeMap<AstLanguageId, AstProviderId>,
    by_extension: BTreeMap<String, AstProviderId>,
}

impl AstProviderRegistry {
    /// Construct an empty [`AstProviderRegistry`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a native AST language provider and bind its language & file extensions.
    pub fn register(
        &mut self,
        mut descriptor: AstProviderDescriptor,
        port: Arc<dyn AstProviderPort>,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        validate_registration_alignment(&descriptor, port.as_ref())?;
        if self.providers.contains_key(&descriptor.provider_id) {
            return Err(AstEngineError::DuplicateProvider(
                descriptor.provider_id.as_str().to_owned(),
            ));
        }
        descriptor.source_extensions = normalize_extensions(&descriptor.source_extensions)?;
        let provider_id = descriptor.provider_id.clone();
        let language_id = descriptor.language_id.clone();
        for ext in &descriptor.source_extensions {
            self.by_extension.insert(ext.clone(), provider_id.clone());
        }
        self.by_language.insert(language_id, provider_id.clone());
        self.providers
            .insert(provider_id, RegisteredAstProvider { descriptor, port });
        self.generation = self.generation.saturating_add(1);
        Ok(self.receipt())
    }

    /// Register a native AST provider directly from ASP's canonical
    /// [`ProviderRegistrationDocument`] (`languageId`, `providerId`, and
    /// `sourceInventory.sourceExtensions`).
    pub fn register_from_provider_document(
        &mut self,
        document: &ProviderRegistrationDocument,
        binding: AstProviderDocumentBinding,
        port: Arc<dyn AstProviderPort>,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        document.validate().map_err(AstEngineError::ProviderError)?;
        let inventory = document
            .source_inventory()
            .map_err(AstEngineError::ProviderError)?;
        let descriptor = AstProviderDescriptor {
            provider_id: AstProviderId::new(document.provider_id.clone())?,
            language_id: AstLanguageId::new(document.language_id.clone())?,
            source_extensions: inventory.source_extensions,
            lexical_profile: binding.lexical_profile,
            capabilities: binding.capabilities,
            transport_kind: binding.transport_kind,
            endpoint: binding.endpoint,
        };
        self.register(descriptor, port)
    }

    /// Register a native AST provider directly from a converged
    /// [`ProviderCoreManifest`] (`provider-core-manifest.json`), deriving the
    /// calibration lexical profile directly from the provider's self-reported
    /// [`agent_semantic_provider_protocol::ProviderSyntaxSurface`].
    pub fn register_from_manifest(
        &mut self,
        manifest: &ProviderCoreManifest,
        transport_kind: AstProviderTransportKind,
        endpoint: Option<String>,
        port: Arc<dyn AstProviderPort>,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        if manifest.schema_id != PROVIDER_CORE_MANIFEST_SCHEMA_ID {
            return Err(AstEngineError::ProviderError(format!(
                "invalid provider core manifest schemaId: {}",
                manifest.schema_id
            )));
        }
        let descriptor = AstProviderDescriptor {
            provider_id: AstProviderId::new(manifest.provider_id.as_str())?,
            language_id: AstLanguageId::new(manifest.language_id.as_str())?,
            source_extensions: manifest.extensions.clone(),
            lexical_profile: LexicalDelimiterProfile::from_syntax_surface(&manifest.syntax_surface),
            capabilities: AstProviderCapabilities {
                ast_inspect: manifest.capabilities.ast_inspect,
                ast_verify: manifest.capabilities.ast_verify,
                open_edb_facts: manifest.capabilities.open_edb,
            },
            transport_kind,
            endpoint,
        };
        self.register(descriptor, port)
    }

    /// Register a loopback HTTP-JSON native language provider directly from its
    /// checked-in [`ProviderCoreManifest`] and live endpoint URL (zero hardcoded
    /// extensions or language lexical profiles at the call site).
    pub fn register_http_from_manifest(
        &mut self,
        manifest: &ProviderCoreManifest,
        endpoint: impl Into<String>,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        let endpoint_str = endpoint.into();
        let provider_id = AstProviderId::new(manifest.provider_id.as_str())?;
        let language_id = AstLanguageId::new(manifest.language_id.as_str())?;
        let lexical_profile =
            LexicalDelimiterProfile::from_syntax_surface(&manifest.syntax_surface);
        let capabilities = AstProviderCapabilities {
            ast_inspect: manifest.capabilities.ast_inspect,
            ast_verify: manifest.capabilities.ast_verify,
            open_edb_facts: manifest.capabilities.open_edb,
        };
        let port = Arc::new(HttpJsonAstProviderClient::new(
            endpoint_str.clone(),
            AstProviderClientConfig {
                provider_id,
                language_id,
                lexical_profile,
                capabilities,
            },
        ));
        self.register_from_manifest(
            manifest,
            AstProviderTransportKind::HttpJsonLoopback,
            Some(endpoint_str),
            port,
        )
    }

    /// Unregister an existing provider and remove its language & extension routes.
    pub fn unregister(
        &mut self,
        provider_id: &AstProviderId,
    ) -> Result<AstProviderRegisterReceipt, AstEngineError> {
        let Some(removed) = self.providers.remove(provider_id) else {
            return Err(AstEngineError::ProviderNotRegistered(
                provider_id.as_str().to_owned(),
            ));
        };
        if self
            .by_language
            .get(&removed.descriptor.language_id)
            .is_some_and(|id| id == provider_id)
        {
            self.by_language.remove(&removed.descriptor.language_id);
        }
        self.by_extension.retain(|_, id| id != provider_id);
        self.generation = self.generation.saturating_add(1);
        Ok(self.receipt())
    }

    /// Resolve the registered provider owning `language_id`.
    pub fn resolve_for_language(
        &self,
        language_id: &AstLanguageId,
    ) -> Result<RegisteredAstProvider, AstEngineError> {
        let provider_id = self.by_language.get(language_id).ok_or_else(|| {
            AstEngineError::ProviderNotRegistered(format!("language `{language_id}`"))
        })?;
        self.providers
            .get(provider_id)
            .cloned()
            .ok_or_else(|| AstEngineError::ProviderNotRegistered(provider_id.as_str().to_owned()))
    }

    /// Resolve the registered provider owning the file extension of `path`.
    pub fn resolve_for_path(
        &self,
        path: &AstWorkspacePath,
    ) -> Result<RegisteredAstProvider, AstEngineError> {
        let ext = path.normalized_extension().ok_or_else(|| {
            AstEngineError::ProviderNotRegistered(format!(
                "path `{}` (no extension)",
                path.as_str()
            ))
        })?;
        let provider_id = self.by_extension.get(&ext).ok_or_else(|| {
            AstEngineError::ProviderNotRegistered(format!(
                "extension `{ext}` for `{}`",
                path.as_str()
            ))
        })?;
        self.providers
            .get(provider_id)
            .cloned()
            .ok_or_else(|| AstEngineError::ProviderNotRegistered(provider_id.as_str().to_owned()))
    }

    /// Resolve by explicit `language_override` if supplied, otherwise route by `path` extension.
    pub fn resolve(
        &self,
        path: &AstWorkspacePath,
        language_override: Option<&AstLanguageId>,
    ) -> Result<RegisteredAstProvider, AstEngineError> {
        match language_override {
            Some(language_id) => self.resolve_for_language(language_id),
            None => self.resolve_for_path(path),
        }
    }

    /// List all registered provider descriptors in canonical sorted order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<AstProviderDescriptor> {
        self.providers
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    /// Return the monotonic registration generation counter.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Compute the canonical BLAKE3 digest of the current registered provider set.
    #[must_use]
    pub fn registry_digest(&self) -> Blake3SourceDigest {
        let encoded = serde_json::to_vec(&self.descriptors()).unwrap_or_default();
        blake3_source_digest(&encoded)
    }

    fn receipt(&self) -> AstProviderRegisterReceipt {
        AstProviderRegisterReceipt {
            generation: self.generation,
            registry_digest: self.registry_digest(),
            registered_count: self.providers.len(),
        }
    }
}

fn validate_registration_alignment(
    descriptor: &AstProviderDescriptor,
    port: &dyn AstProviderPort,
) -> Result<(), AstEngineError> {
    if descriptor.provider_id != *port.provider_id()
        || descriptor.language_id != *port.language_id()
    {
        return Err(AstEngineError::ProviderError(format!(
            "provider registration drift: descriptor={}/{}, port={}/{}",
            descriptor.language_id,
            descriptor.provider_id,
            port.language_id(),
            port.provider_id()
        )));
    }
    if !descriptor.capabilities.ast_inspect || !descriptor.capabilities.ast_verify {
        return Err(AstEngineError::ProviderError(
            "AST provider registration requires astInspect and astVerify capabilities".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_extensions(raw: &[String]) -> Result<Vec<String>, AstEngineError> {
    if raw.is_empty() {
        return Err(AstEngineError::InvalidIdentifier(
            "provider source extensions must not be empty",
        ));
    }
    let mut normalized = Vec::with_capacity(raw.len());
    for ext in raw {
        let trimmed = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if trimmed.is_empty() {
            return Err(AstEngineError::InvalidIdentifier("source extension"));
        }
        normalized.push(format!(".{trimmed}"));
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}
