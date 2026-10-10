// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider-published capability proof for resident enhanced Tree-sitter Query.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const ENHANCED_QUERY_CAPABILITY_TABLE_SCHEMA_ID: &str =
    "agent.semantic-protocols.enhanced-tree-sitter-query-capability-table";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnhancedQueryCapabilityTable {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub parser_abi: EnhancedQueryVersionedIdentity,
    pub query_grammar: EnhancedQueryVersionedIdentity,
    pub operator_table_digest: String,
    pub table_digest: String,
    pub rows: Vec<EnhancedQueryCapabilityRow>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnhancedQueryVersionedIdentity {
    pub id: String,
    pub version: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnhancedQueryCapabilityRow {
    pub row_id: String,
    pub kind: EnhancedQueryCapabilityRowKind,
    pub source_name: String,
    pub publication_state: EnhancedQueryPublicationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lowering: Option<EnhancedQueryCapabilityLowering>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equivalence_evidence: Option<EnhancedQueryEquivalenceEvidence>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnhancedQueryCapabilityRowKind {
    NodeType,
    Field,
    CaptureBinding,
    FactPath,
    ProjectionKind,
    RelationKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnhancedQueryPublicationState {
    Runtime,
    ProviderLocal,
    NotMaterialized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnhancedQueryCapabilityLowering {
    pub resident_fact_path: crate::ResidentSyntaxQueryFactPath,
    pub constraint_kind: EnhancedQueryConstraintKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resident_value: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnhancedQueryConstraintKind {
    Scalar,
    Set,
    Range,
    Relation,
    Capture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnhancedQueryEquivalenceEvidence {
    pub method: String,
    pub corpus_digest: String,
    pub receipt_digest: String,
}

impl EnhancedQueryCapabilityTable {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != ENHANCED_QUERY_CAPABILITY_TABLE_SCHEMA_ID
            || self.schema_version != "1"
            || self.language_id.is_empty()
            || self.provider_id.is_empty()
            || self.rows.is_empty()
        {
            return Err("enhanced Query capability table identity is incomplete".to_owned());
        }
        for (name, digest) in [
            ("parserAbi.digest", self.parser_abi.digest.as_str()),
            ("queryGrammar.digest", self.query_grammar.digest.as_str()),
            ("operatorTableDigest", self.operator_table_digest.as_str()),
            ("tableDigest", self.table_digest.as_str()),
        ] {
            validate_digest(name, digest)?;
        }
        if self.parser_abi.id.is_empty()
            || self.parser_abi.version.is_empty()
            || self.query_grammar.id.is_empty()
            || self.query_grammar.version.is_empty()
        {
            return Err("enhanced Query capability identity is incomplete".to_owned());
        }
        let row_ids = self
            .rows
            .iter()
            .map(|row| row.row_id.as_str())
            .collect::<BTreeSet<_>>();
        if row_ids.len() != self.rows.len() {
            return Err("enhanced Query capability row ids are not unique".to_owned());
        }
        for row in &self.rows {
            if row.row_id.is_empty() || row.source_name.is_empty() {
                return Err("enhanced Query capability row identity is incomplete".to_owned());
            }
            match row.publication_state {
                EnhancedQueryPublicationState::Runtime => {
                    let lowering = row.lowering.as_ref().ok_or_else(|| {
                        "runtime enhanced Query capability row lacks lowering".to_owned()
                    })?;
                    let evidence = row.equivalence_evidence.as_ref().ok_or_else(|| {
                        "runtime enhanced Query capability row lacks equivalence evidence"
                            .to_owned()
                    })?;
                    if evidence.method != "native-parser-tree-sitter-differential-v1" {
                        return Err(
                            "enhanced Query capability evidence method is unsupported".to_owned()
                        );
                    }
                    validate_digest("equivalenceEvidence.corpusDigest", &evidence.corpus_digest)?;
                    validate_digest(
                        "equivalenceEvidence.receiptDigest",
                        &evidence.receipt_digest,
                    )?;
                    if row.kind == EnhancedQueryCapabilityRowKind::NodeType
                        && (lowering.constraint_kind != EnhancedQueryConstraintKind::Scalar
                            || lowering.resident_value.as_deref().is_none_or(str::is_empty))
                    {
                        return Err(
                            "runtime node capability must bind one resident scalar value"
                                .to_owned(),
                        );
                    }
                }
                EnhancedQueryPublicationState::ProviderLocal
                | EnhancedQueryPublicationState::NotMaterialized => {
                    if row.equivalence_evidence.is_some() {
                        return Err(
                            "non-runtime capability row cannot claim equivalence evidence"
                                .to_owned(),
                        );
                    }
                }
            }
        }
        self.validate_table_digest()
    }

    pub fn runtime_row(
        &self,
        kind: EnhancedQueryCapabilityRowKind,
        source_name: &str,
    ) -> Result<&EnhancedQueryCapabilityRow, String> {
        let row = self
            .rows
            .iter()
            .find(|row| row.kind == kind && row.source_name == source_name)
            .ok_or_else(|| {
                format!(
                    "enhanced-query-capability-row-missing kind={kind:?} sourceName={source_name}"
                )
            })?;
        if row.publication_state != EnhancedQueryPublicationState::Runtime {
            return Err(format!(
                "enhanced-query-capability-not-runtime rowId={} state={:?}",
                row.row_id, row.publication_state
            ));
        }
        Ok(row)
    }

    fn validate_table_digest(&self) -> Result<(), String> {
        let mut value = serde_json::to_value(self)
            .map_err(|error| format!("encode enhanced Query capability table: {error}"))?;
        value
            .as_object_mut()
            .expect("capability table serializes as an object")
            .remove("tableDigest");
        let canonical = crate::canonical_json::to_jcs_vec(&value)
            .map_err(|error| format!("canonicalize enhanced Query capability table: {error}"))?;
        let actual = format!("blake3-256:{}", blake3::hash(&canonical).to_hex());
        if actual != self.table_digest {
            return Err(format!(
                "enhanced Query capability table digest mismatch: expected={} actual={actual}",
                self.table_digest
            ));
        }
        Ok(())
    }
}

fn validate_digest(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return Err(format!("enhanced Query {name} is not blake3-256"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("enhanced Query {name} is malformed"));
    }
    Ok(())
}
