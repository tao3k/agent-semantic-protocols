// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable lookup indexes for bounded Search projection over one topology.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{
    ProjectTopologyLibrary, ProjectTopologyLibraryError, invalid, required_array, required_object,
    required_text,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ProjectTopologySearchProjectionIndex {
    node_by_selector: BTreeMap<String, usize>,
    node_by_id: BTreeMap<String, usize>,
    incident_edges_by_node: BTreeMap<String, Vec<usize>>,
}

impl ProjectTopologySearchProjectionIndex {
    pub(super) fn build(packet: &Value) -> Result<Self, ProjectTopologyLibraryError> {
        let object = required_object(packet, "packet")?;
        let mut index = Self::default();
        for (position, node) in required_array(object, "nodes")?.iter().enumerate() {
            let node = required_object(node, "nodes[]")?;
            let id = required_text(node, "id")?;
            index.node_by_id.insert(id.to_owned(), position);
            if let Some(selector) = node.get("selector").and_then(Value::as_str)
                && index
                    .node_by_selector
                    .insert(selector.to_owned(), position)
                    .is_some()
            {
                return invalid(
                    "topology-duplicate-selector",
                    format!("duplicate topology selector {selector}"),
                );
            }
        }
        for (position, edge) in required_array(object, "edges")?.iter().enumerate() {
            let edge = required_object(edge, "edges[]")?;
            for endpoint in [required_text(edge, "from")?, required_text(edge, "to")?] {
                index
                    .incident_edges_by_node
                    .entry(endpoint.to_owned())
                    .or_default()
                    .push(position);
            }
        }
        Ok(index)
    }
}

impl ProjectTopologyLibrary {
    /// Resolves one exact selector through the generation-owned immutable
    /// Search projection index.
    pub fn search_node_by_selector(&self, selector: &str) -> Option<&Value> {
        let index = *self
            .search_projection_index
            .node_by_selector
            .get(selector)?;
        self.packet.get("nodes")?.as_array()?.get(index)
    }

    /// Returns selected nodes in canonical packet order without scanning the
    /// complete topology generation.
    pub fn search_nodes_by_id<'a>(&'a self, ids: &BTreeSet<String>) -> Vec<&'a Value> {
        let Some(nodes) = self.packet.get("nodes").and_then(Value::as_array) else {
            return Vec::new();
        };
        let mut indices = ids
            .iter()
            .filter_map(|id| self.search_projection_index.node_by_id.get(id).copied())
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices
            .into_iter()
            .filter_map(|index| nodes.get(index))
            .collect()
    }

    /// Returns the union of edges incident to an exact node set in canonical
    /// packet order. The work is proportional to the selected neighborhood.
    pub fn search_incident_edges<'a>(&'a self, ids: &BTreeSet<String>) -> Vec<&'a Value> {
        let Some(edges) = self.packet.get("edges").and_then(Value::as_array) else {
            return Vec::new();
        };
        let mut indices = BTreeSet::new();
        for id in ids {
            if let Some(incident) = self.search_projection_index.incident_edges_by_node.get(id) {
                indices.extend(incident.iter().copied());
            }
        }
        indices
            .into_iter()
            .filter_map(|index| edges.get(index))
            .collect()
    }
}
