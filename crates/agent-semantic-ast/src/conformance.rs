// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Data-driven black-box Provider Conformance Harness (RFC 10.36).
//!
//! Consumes compact [`LanguageConformancePack`] payloads projected from Marlin's
//! Gerbil Scheme `defasp-language` POO General Ontology and verifies any external
//! native language provider over `POST /v1/provider-runtime` (`ast-inspect` +
//! `ast-verify`), asserting:
//!   1. `ast-verify` matches every probe's `expect_valid` truth value.
//!   2. Valid probes pass `ast-inspect` with UTF-8 char-boundary `ByteSpan`s.
//!   3. The native provider self-reports its [`ProviderSyntaxSurface`] (comment
//!      syntax, delimiter pairs, string quotes) rather than duplicating lexical
//!      rules inside Scheme.
//!   4. Every expected `(kind, language_kind, name)` entity is emitted accurately.

use agent_semantic_provider_protocol::{
    ExpectedConformanceNode, LanguageConformancePack, ProviderSyntaxSurface,
};

use crate::provider_port::{
    AstInspectRequest, AstProviderPort, AstVerifyCandidate, AstVerifyRequest,
};
use crate::span_overlay::blake3_source_digest;
use crate::transaction::AstEngine;
use crate::types::{AstEngineError, AstLanguageId, AstWorkspacePath};

/// Receipt emitted by [`run_language_conformance_pack`] for one verified probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceProbeReceipt {
    pub probe_id: String,
    pub passed: bool,
    pub observed_node_count: usize,
    pub failure_reason: Option<String>,
}

/// Summary report returned after verifying one registered native provider against
/// a Scheme-projected [`LanguageConformancePack`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageConformanceReport {
    pub language_id: String,
    pub provider_language_id: String,
    pub syntax_surface: Option<ProviderSyntaxSurface>,
    pub probes: Vec<ConformanceProbeReceipt>,
}

impl LanguageConformanceReport {
    /// True iff every probe passed and the provider reported its native syntax surface.
    #[must_use]
    pub fn is_conformant(&self) -> bool {
        self.syntax_surface.is_some() && self.probes.iter().all(|probe| probe.passed)
    }
}

fn validate_expected_nodes(
    source: &str,
    expected: &[ExpectedConformanceNode],
    nodes: &[crate::types::AstNodeSpanRecord],
) -> Option<String> {
    for node in nodes {
        let s = node.outer_span.start;
        let e = node.outer_span.end;
        if s > e
            || e > source.len()
            || !source.is_char_boundary(s)
            || !source.is_char_boundary(e)
        {
            return Some(format!(
                "invalid UTF-8 outer_span [{s}, {e}) for node {}",
                node.locator
            ));
        }
    }
    for want in expected {
        let found = nodes.iter().any(|got| {
            got.kind.as_open_edb_kind() == want.kind
                && got.language_kind.as_str() == want.language_kind
                && got.name == want.name
        });
        if !found {
            return Some(format!(
                "missing expected AST node ({}, {}, {})",
                want.kind, want.language_kind, want.name
            ));
        }
    }
    None
}

fn evaluate_single_probe(
    provider: &dyn AstProviderPort,
    probe: &agent_semantic_provider_protocol::LanguageConformanceProbe,
    observed_surface: &mut Option<ProviderSyntaxSurface>,
) -> Result<ConformanceProbeReceipt, AstEngineError> {
    let path = AstWorkspacePath::new(&probe.path)?;
    let verify_resp = provider.verify(&AstVerifyRequest {
        path: path.clone(),
        candidates: vec![AstVerifyCandidate {
            candidate_id: probe.probe_id.clone(),
            source_utf8: probe.source.clone(),
        }],
    })?;
    let outcome = verify_resp.results.first().ok_or_else(|| {
        AstEngineError::ProviderError(format!(
            "ast-verify returned empty results for probe {}",
            probe.probe_id
        ))
    })?;
    if outcome.accepted != probe.expect_valid {
        return Ok(ConformanceProbeReceipt {
            probe_id: probe.probe_id.clone(),
            passed: false,
            observed_node_count: 0,
            failure_reason: Some(format!(
                "ast-verify accepted={} != expected {}",
                outcome.accepted, probe.expect_valid
            )),
        });
    }
    if !probe.expect_valid {
        return Ok(ConformanceProbeReceipt {
            probe_id: probe.probe_id.clone(),
            passed: true,
            observed_node_count: 0,
            failure_reason: None,
        });
    }
    let inspect_resp = provider.inspect(&AstInspectRequest {
        path,
        source_utf8: probe.source.clone(),
        source_hash: blake3_source_digest(probe.source.as_bytes()),
    })?;
    if observed_surface.is_none() {
        *observed_surface = inspect_resp.syntax_surface.clone();
    }
    if !inspect_resp.accepted {
        return Ok(ConformanceProbeReceipt {
            probe_id: probe.probe_id.clone(),
            passed: false,
            observed_node_count: inspect_resp.nodes.len(),
            failure_reason: Some("ast-inspect rejected valid probe source".to_owned()),
        });
    }
    let node_err = validate_expected_nodes(&probe.source, &probe.expected_nodes, &inspect_resp.nodes);
    Ok(ConformanceProbeReceipt {
        probe_id: probe.probe_id.clone(),
        passed: node_err.is_none(),
        observed_node_count: inspect_resp.nodes.len(),
        failure_reason: node_err,
    })
}

/// Run a Scheme-projected [`LanguageConformancePack`] against the matching provider
/// registered in [`AstEngine`].
pub fn run_language_conformance_pack(
    engine: &AstEngine,
    pack: &LanguageConformancePack,
) -> Result<LanguageConformanceReport, AstEngineError> {
    let lang_id = AstLanguageId::new(pack.provider_language_id.as_str())?;
    let registered = engine.providers().resolve_for_language(&lang_id)?;
    let mut syntax_surface = None;
    let probes = pack
        .probes
        .iter()
        .map(|probe| evaluate_single_probe(registered.port.as_ref(), probe, &mut syntax_surface))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LanguageConformanceReport {
        language_id: pack.language_id.as_str().to_owned(),
        provider_language_id: pack.provider_language_id.as_str().to_owned(),
        syntax_surface,
        probes,
    })
}
