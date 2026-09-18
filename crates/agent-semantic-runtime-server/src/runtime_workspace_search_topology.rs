// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident Topology owner-set execution for the Workspace Search Playbook.

use std::collections::BTreeSet;

use crate::RuntimeQueryGeneration;

use super::AspClientOperationError;

pub(super) struct TopologyClauseResult {
    pub owners: Vec<String>,
    pub syntax_candidates: Vec<agent_semantic_search::WorkspaceSearchSyntaxCandidate>,
    pub truncated: bool,
}

pub(super) fn execute_topology_block(
    block: &agent_semantic_client_protocol::AspClientSearchPlaybookTopologyBlock,
    routes: &[agent_semantic_search::WorkspaceSearchPlaybookRoute],
    generation: &RuntimeQueryGeneration,
    limit: usize,
) -> Result<TopologyClauseResult, AspClientOperationError> {
    let query = agent_semantic_topology::TopologyOwnerQueryV1 {
        exact_path: block.exact_path.as_deref(),
        path_prefix: block.path_prefix.as_deref(),
        extension: block.extension.as_deref(),
        path_glob: block.path_glob.as_deref(),
    };
    let workspace_owner_count = generation.resident().indexed_owner_count();
    let (owners, index_truncated) = generation
        .resident()
        .read_topology_owner_membership(&query, workspace_owner_count.max(1))
        .map_err(AspClientOperationError::Message)?;
    if index_truncated {
        return Err(AspClientOperationError::Message(
            "resident Topology Index truncated below the workspace owner count".to_owned(),
        ));
    }
    let mut admitted = owners
        .into_iter()
        .filter_map(|owner| {
            topology_owner_language(&owner, routes)
                .transpose()
                .map(|language| language.map(|language| (owner, language)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let truncated = admitted.len() > limit;
    admitted.truncate(limit);
    let owners = admitted
        .iter()
        .map(|(owner, _)| owner.clone())
        .collect::<Vec<_>>();
    let syntax_candidates = admitted
        .into_iter()
        .map(
            |(owner, language)| agent_semantic_search::WorkspaceSearchSyntaxCandidate {
                selector: format!("{language}://{owner}"),
                owner,
                relation: "topology-owner-membership".to_owned(),
                hit: agent_semantic_search::WorkspaceSearchHitProjection {
                    native: true,
                    ..Default::default()
                },
            },
        )
        .collect();
    Ok(TopologyClauseResult {
        owners,
        syntax_candidates,
        truncated,
    })
}

pub(super) fn topology_owner_language(
    owner: &str,
    routes: &[agent_semantic_search::WorkspaceSearchPlaybookRoute],
) -> Result<Option<String>, AspClientOperationError> {
    let extension = owner
        .rsplit_once('.')
        .and_then(|(_, extension)| (!extension.contains('/')).then_some(extension));
    let mut languages = routes
        .iter()
        .filter(|route| {
            extension.is_some_and(|extension| {
                route.extensions.iter().any(|candidate| {
                    candidate
                        .trim_start_matches('.')
                        .eq_ignore_ascii_case(extension)
                })
            })
        })
        .map(|route| route.language_id.as_str())
        .collect::<BTreeSet<_>>();
    match languages.len() {
        0 => Ok(None),
        1 => Ok(languages.pop_first().map(str::to_owned)),
        _ => Err(AspClientOperationError::Message(format!(
            "Topology owner producer is ambiguous: owner={owner}"
        ))),
    }
}
