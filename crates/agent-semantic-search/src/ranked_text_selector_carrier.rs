// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact-selector carrier for ranked-text search.
//!
//! Owner membership is never promoted into parser item membership. Only
//! content-bound parser-native Topology Index hits enter this projection.

use std::collections::BTreeSet;

use crate::{WorkspaceSearchHitProjection, WorkspaceSearchSyntaxCandidate};

pub fn ranked_text_topology_selector_candidates(
    expression: &str,
    owner_scope: &BTreeSet<String>,
    admitted_languages: &BTreeSet<String>,
    hits: impl IntoIterator<Item = agent_semantic_topology::TopologyHitV1>,
    selector_limit: usize,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, String> {
    agent_semantic_topology::ranked_text_topology_selector_carrier(
        owner_scope,
        admitted_languages,
        hits,
        selector_limit,
    )
    .map(|hits| {
        hits.into_iter()
            .map(|hit| WorkspaceSearchSyntaxCandidate {
                owner: hit.owner_path,
                selector: hit.structural_selector,
                relation: "native-parser:tantivy-topology".to_owned(),
                hit: WorkspaceSearchHitProjection {
                    native: true,
                    tantivy: vec![expression.to_owned()],
                    ..Default::default()
                },
            })
            .collect()
    })
}
