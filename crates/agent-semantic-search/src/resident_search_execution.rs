// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Query planning for one ordered resident Search generation.
//!
//! The content generation is constructed once from source-byte acquisition,
//! provider-native syntax, and the Rust resident graph. Tantivy and Python
//! Graph are independently admitted accelerators bound to that content
//! identity.

/// Semantic intent that selects one fixed Search Playbook projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidentSearchIntent {
    /// Recall definitions and concepts.
    Conceptual,
    /// Traverse dependency and reference relationships.
    Relationship,
    /// Verify one exact byte-level literal.
    ExactLiteral,
    /// Prove absence against complete byte coverage.
    AbsenceProof,
}

impl ResidentSearchIntent {
    /// Parses the stable public intent vocabulary.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "conceptual" => Ok(Self::Conceptual),
            "relationship" => Ok(Self::Relationship),
            "exact-literal" => Ok(Self::ExactLiteral),
            "absence-proof" => Ok(Self::AbsenceProof),
            _ => Err(format!(
                "search intent must be conceptual, relationship, exact-literal, or absence-proof: {value}"
            )),
        }
    }

    /// Returns the stable public intent spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conceptual => "conceptual",
            Self::Relationship => "relationship",
            Self::ExactLiteral => "exact-literal",
            Self::AbsenceProof => "absence-proof",
        }
    }
}

/// Generation-bound availability of each Search Playbook evidence source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentSearchFusionCapabilities {
    /// Tantivy lexical accelerator is open for this generation.
    pub lexical: bool,
    /// Resident native graph is available.
    pub resident_graph: bool,
    /// Python Graph projection is available.
    pub python_graph: bool,
    /// Exact source bytes are available for verification.
    pub byte_evidence: bool,
    /// Byte inventory covers the complete admitted generation.
    pub complete_byte_coverage: bool,
}

impl ResidentSearchFusionCapabilities {
    /// Derive the cold-searchable capability floor from one published content
    /// generation. Derived attachments are intentionally absent here: Runtime
    /// adds only attachment facts observed for this exact generation.
    pub fn from_open_generation(
        receipt: &crate::ContentSearchGenerationReceipt,
    ) -> Result<Self, String> {
        receipt.validate()?;
        Ok(Self {
            lexical: false,
            resident_graph: false,
            python_graph: false,
            byte_evidence: true,
            complete_byte_coverage: true,
        })
    }
}

/// Fixed execution plan derived from intent and generation-bound capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentSearchExecutionPlan {
    /// Requested semantic intent.
    pub intent: ResidentSearchIntent,
    /// Whether Tantivy contributes lexical candidates.
    pub use_tantivy_candidates: bool,
    /// Whether the one-process cold `rg` lane supplies lexical candidates.
    pub use_cold_rg_candidates: bool,
    /// Whether native resident graph facts are projected.
    pub project_resident_graph: bool,
    /// Whether Python Graph is required for admission.
    pub require_python_graph: bool,
    /// Whether final results must be verified against exact source bytes.
    pub verify_rg_bytes: bool,
}

/// Produces the deterministic Search Playbook plan or a typed not-ready error.
pub fn plan_resident_search_execution(
    intent: ResidentSearchIntent,
    capabilities: ResidentSearchFusionCapabilities,
) -> Result<ResidentSearchExecutionPlan, String> {
    if !capabilities.byte_evidence {
        return Err(
            "query-not-ready: Search playbook requires a published content generation with byte evidence"
                .to_owned(),
        );
    }
    if intent == ResidentSearchIntent::AbsenceProof && !capabilities.complete_byte_coverage {
        return Err(
            "query-not-ready: absence proof requires complete generation-bound byte coverage"
                .to_owned(),
        );
    }
    if intent == ResidentSearchIntent::Relationship && !capabilities.python_graph {
        return Err(
            "query-not-ready: relationship intent requires an exact generation-bound Python Graph capability"
                .to_owned(),
        );
    }

    let lexical_intent = matches!(
        intent,
        ResidentSearchIntent::Conceptual | ResidentSearchIntent::Relationship
    );
    Ok(ResidentSearchExecutionPlan {
        intent,
        use_tantivy_candidates: lexical_intent && capabilities.lexical,
        use_cold_rg_candidates: lexical_intent && !capabilities.lexical,
        project_resident_graph: capabilities.resident_graph,
        require_python_graph: intent == ResidentSearchIntent::Relationship,
        verify_rg_bytes: matches!(
            intent,
            ResidentSearchIntent::ExactLiteral | ResidentSearchIntent::AbsenceProof
        ),
    })
}
