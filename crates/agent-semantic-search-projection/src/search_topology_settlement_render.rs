// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compact Agent-facing Org/GQL rendering for admitted Search settlements.

use std::collections::BTreeMap;

use orgize::ast::{
    OrgSourceBlock, OrgSourceBlockDocument, OrgSourceBlockHeader, OrgSourceBlockHeaderValue,
};
use rustc_hash::FxHashSet;
use serde_json::Value;

use super::{SearchTopologySettlement, SearchTopologySettlementError};
use crate::search_topology_settlement_error::error;
use crate::search_topology_settlement_support::{
    gql_alias, is_search_result_support_edge, org_render_error, public_node_aliases, quoted,
    render_edge, render_node, render_result_node, required_array, required_object,
    required_object_field, required_text, required_u64,
};

impl SearchTopologySettlement {
    /// Render the admitted settlement as the single Agent-facing Org/GQL block.
    pub fn render_org_gql(&self) -> Result<String, SearchTopologySettlementError> {
        let packet = required_object(&self.packet, "packet")?;
        let mut lines = Vec::new();
        let mut result_nodes = BTreeMap::<String, Vec<String>>::new();
        let mut standalone_nodes = Vec::new();

        let result_state = required_text(packet, "resultState")?;
        let inference = required_object_field(packet, "inference")?;
        let closure_state = required_text(inference, "state")?;
        let terminal = required_object_field(packet, "terminal")?;
        let mut closure_properties = vec![
            format!("state:{}", quoted(closure_state)),
            format!("result:{}", quoted(result_state)),
            format!("queryable:{}", result_state == "queryable"),
            format!("terminal:{}", quoted(required_text(terminal, "state")?)),
        ];
        if let Some(reason_kind) = terminal.get("reasonKind").and_then(Value::as_str) {
            closure_properties.push(format!("reason:{}", quoted(reason_kind)));
        }
        lines.push(format!(
            "(evidence:SearchEvidence {{{}}})",
            closure_properties.join(",")
        ));

        let nodes = required_array(packet, "nodes")?;
        let ranked_node_ids = nodes
            .iter()
            .filter_map(|node| {
                let node = node.as_object()?;
                node.contains_key("projection")
                    .then(|| node.get("id").and_then(Value::as_str))
                    .flatten()
            })
            .collect::<FxHashSet<_>>();
        let edges = required_array(packet, "edges")?;
        let mut result_support_only_owner_ids = FxHashSet::default();
        let mut independently_visible_node_ids = FxHashSet::default();
        for edge in edges {
            let edge = required_object(edge, "edges[]")?;
            if is_search_result_support_edge(edge, &ranked_node_ids) {
                result_support_only_owner_ids.insert(required_text(edge, "from")?);
            } else {
                independently_visible_node_ids.insert(required_text(edge, "from")?);
                independently_visible_node_ids.insert(required_text(edge, "to")?);
            }
        }
        for frontier in required_array(packet, "frontiers")? {
            let frontier = required_object(frontier, "frontiers[]")?;
            independently_visible_node_ids.insert(required_text(frontier, "anchor")?);
            independently_visible_node_ids.insert(required_text(frontier, "target")?);
        }
        let mut visible_node_ids = FxHashSet::default();
        for node in nodes {
            let node = required_object(node, "nodes[]")?;
            let node_id = required_text(node, "id")?;
            let visible = node.contains_key("projection")
                || node.contains_key("annotation")
                || node.contains_key("excerpt")
                || !result_support_only_owner_ids.contains(node_id)
                || independently_visible_node_ids.contains(node_id);
            if visible {
                visible_node_ids.insert(node_id);
            }
        }
        let public_aliases = public_node_aliases(nodes, &visible_node_ids)?;

        for node in nodes {
            let node = required_object(node, "nodes[]")?;
            let node_id = required_text(node, "id")?;
            if !visible_node_ids.contains(node_id) {
                continue;
            }
            let alias = public_aliases
                .get(node_id)
                .ok_or_else(|| error("rendering-failed", "visible node has no public alias"))?;
            if node.contains_key("annotation") || node.contains_key("excerpt") {
                standalone_nodes.push(render_node(node, alias)?);
            } else {
                let (language, rendered) = render_result_node(node, alias)?;
                result_nodes.entry(language).or_default().push(rendered);
            }
        }
        for (language, nodes) in result_nodes {
            lines.push(format!(
                "({}:Language)-[:RESULTS]->[{}]",
                gql_alias(&language),
                nodes.join(",")
            ));
        }
        lines.extend(standalone_nodes);
        for edge in edges {
            let edge = required_object(edge, "edges[]")?;
            if is_search_result_support_edge(edge, &ranked_node_ids) {
                continue;
            }
            let from = required_text(edge, "from")?;
            let to = required_text(edge, "to")?;
            if visible_node_ids.contains(from) && visible_node_ids.contains(to) {
                let from_alias = public_aliases.get(from).ok_or_else(|| {
                    error(
                        "rendering-failed",
                        "visible edge source has no public alias",
                    )
                })?;
                let to_alias = public_aliases.get(to).ok_or_else(|| {
                    error(
                        "rendering-failed",
                        "visible edge target has no public alias",
                    )
                })?;
                lines.push(render_edge(edge, from_alias, to_alias)?);
            }
        }
        for certificate in required_array(packet, "coverageCertificates")? {
            let certificate = required_object(certificate, "coverageCertificates[]")?;
            lines.push(format!(
                "({}:CoverageCertificate {{relation:{},target_kind:{},scope:{},digest:{}}})",
                gql_alias(required_text(certificate, "id")?),
                quoted(required_text(certificate, "relation")?),
                quoted(required_text(certificate, "targetKind")?),
                quoted(required_text(certificate, "scope")?),
                quoted(required_text(certificate, "digest")?),
            ));
        }
        for frontier in required_array(packet, "frontiers")? {
            let frontier = required_object(frontier, "frontiers[]")?;
            let mut properties = vec![
                format!("relation:{}", quoted(required_text(frontier, "relation")?)),
                format!(
                    "target_kind:{}",
                    quoted(required_text(frontier, "targetKind")?)
                ),
                format!("depth:{}", required_u64(frontier, "depth")?),
                format!("state:{}", quoted(required_text(frontier, "state")?)),
                format!("reason:{}", quoted(required_text(frontier, "reason")?)),
            ];
            if let Some(reference) = frontier.get("coverageRef").and_then(Value::as_str) {
                properties.push(format!("coverage:{}", quoted(reference)));
            }
            lines.push(format!(
                "({})-[:FRONTIER {{{}}}]->({})",
                public_aliases
                    .get(required_text(frontier, "anchor")?)
                    .ok_or_else(|| error(
                        "rendering-failed",
                        "frontier anchor has no public alias"
                    ))?,
                properties.join(","),
                public_aliases
                    .get(required_text(frontier, "target")?)
                    .ok_or_else(|| error(
                        "rendering-failed",
                        "frontier target has no public alias"
                    ))?,
            ));
        }
        let block = OrgSourceBlock::new(
            "gql",
            vec![
                OrgSourceBlockHeader::new(
                    "name",
                    OrgSourceBlockHeaderValue::token("result").map_err(org_render_error)?,
                )
                .map_err(org_render_error)?,
            ],
            vec![],
            lines.join("\n"),
        )
        .map_err(org_render_error)?;
        OrgSourceBlockDocument::new(vec![block])
            .and_then(|document| document.render())
            .map_err(org_render_error)
    }
}
