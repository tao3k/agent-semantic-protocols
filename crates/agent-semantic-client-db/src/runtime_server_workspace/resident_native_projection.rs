// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Parser-owned native syntax projection over a resident overlay snapshot.

use std::collections::BTreeSet;

use super::WorkspaceMemoryGeneration;
use super::resident_overlay::ResidentOverlaySnapshot;

impl ResidentOverlaySnapshot {
    #[expect(
        clippy::type_complexity,
        reason = "the V1 projection returns its three typed evidence collections"
    )]
    pub(super) fn native_syntax_playbook_projection(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_paths: &[String],
    ) -> Result<
        (
            Vec<agent_semantic_search::NativeSyntaxProjection>,
            Vec<agent_semantic_search::NativeSyntaxRelation>,
            Vec<agent_semantic_search::NativeSyntaxDiagnostic>,
        ),
        String,
    > {
        let admitted = owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut projections = Vec::with_capacity(admitted.len());
        let mut diagnostics = Vec::new();
        let mut relations = Vec::new();
        for owner_path in admitted {
            let owner = self
                .owner_snapshot(base, owner_path)
                .ok_or_else(|| "native syntax playbook owner is absent".to_owned())?;
            if let Some(diagnostic) = owner.native_syntax_diagnostic {
                diagnostics.push(diagnostic);
                continue;
            }
            if owner.selectors.is_empty() && !self.state.semantic_owners.contains(owner_path) {
                diagnostics.push(agent_semantic_search::NativeSyntaxDiagnostic {
                    owner_path: owner.owner_path,
                    content_digest: owner.content_digest,
                    reason_kind: "source-syntax-unavailable".to_owned(),
                    message: "the admitted owner has no parser-owned selectors".to_owned(),
                });
                continue;
            }
            let selectors = owner
                .selectors
                .iter()
                .map(|selector| {
                    let encoded =
                        serde_json::to_vec(&selector.derived_projections).map_err(|error| {
                            format!("encode resident native syntax projections: {error}")
                        })?;
                    Ok(agent_semantic_search::NativeSyntaxSelector {
                        selector: selector.selector.clone(),
                        byte_start: selector.byte_start,
                        byte_end: selector.byte_end,
                        query_keys: selector.query_keys.clone(),
                        derived_projection_digest: format!(
                            "blake3-256:{}",
                            blake3::hash(&encoded).to_hex()
                        ),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            for relation in self.owner_relations(base, owner_path) {
                let encoded = serde_json::to_vec(&relation.relation)
                    .map_err(|error| format!("encode resident native syntax relation: {error}"))?;
                relations.push(agent_semantic_search::NativeSyntaxRelation {
                    owner_path: owner_path.to_owned(),
                    relation_digest: format!("blake3-256:{}", blake3::hash(&encoded).to_hex()),
                });
            }
            projections.push(agent_semantic_search::NativeSyntaxProjection {
                owner_path: owner.owner_path,
                content_digest: owner.content_digest,
                selectors,
            });
        }
        projections.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        diagnostics.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        relations.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.relation_digest.cmp(&right.relation_digest))
        });
        Ok((projections, relations, diagnostics))
    }
}
