// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Agent-prioritized progressive plan for the public Search Playbook.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceSearchProducerAxis {
    Language,
    Document,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProvider {
    pub language_id: String,
    pub provider_id: String,
    pub source_extensions: Vec<String>,
    pub search_supported: bool,
    pub producer_axes: Vec<WorkspaceSearchProducerAxis>,
    pub enhanced_query_capability:
        Option<agent_semantic_client_protocol::EnhancedQueryCapabilityTable>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchPlanBinding {
    pub project_id: String,
    pub workspace_id: String,
    pub content_generation_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchPlaybookRoute {
    pub language_id: String,
    pub provider_id: String,
    pub producer_axis: WorkspaceSearchProducerAxis,
    pub generation_digest: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProgressivePlan {
    pub rg: Vec<Vec<String>>,
    pub tantivy: Vec<Vec<String>>,
    pub syntax: Vec<agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock>,
    pub native_syntax: Vec<String>,
    pub graph: Vec<crate::GraphNativeBlock>,
    pub clause_order: Vec<crate::SearchPlaybookClauseRef>,
}

/// Plan-only normalized request after enhanced Query compilation/admission.
/// Public Scheme source belongs to the Client-side source plane and cannot be
/// stored in this scheduler input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedWorkspaceSearchPlaybookRequest {
    pub language: Option<String>,
    pub documents: Option<String>,
    pub workspace: Option<String>,
    pub rg: Vec<Vec<String>>,
    pub tantivy: Vec<Vec<String>>,
    pub syntax: Vec<agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock>,
    pub native_syntax: Vec<String>,
    pub graph: Vec<crate::GraphNativeBlock>,
    pub clause_order: Vec<crate::SearchPlaybookClauseRef>,
}

/// Internal scheduler input. This type is deliberately not serializable and
/// can never be returned as a public Search terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchPlaybookPlan {
    pub project_id: String,
    pub workspace_id: String,
    pub content_generation_digest: String,
    pub language: Option<String>,
    pub documents: Option<String>,
    pub routes: Vec<WorkspaceSearchPlaybookRoute>,
    pub axes: WorkspaceSearchProgressivePlan,
}

pub fn build_workspace_search_playbook_plan(
    request: &NormalizedWorkspaceSearchPlaybookRequest,
    binding: WorkspaceSearchPlanBinding,
    providers: impl IntoIterator<Item = WorkspaceSearchProvider>,
) -> Result<WorkspaceSearchPlaybookPlan, String> {
    let NormalizedWorkspaceSearchPlaybookRequest {
        language,
        documents,
        workspace,
        rg,
        tantivy,
        syntax,
        native_syntax,
        graph,
        clause_order,
    } = request;
    if binding.project_id.is_empty()
        || binding.workspace_id.is_empty()
        || binding.content_generation_digest.is_empty()
    {
        return Err("Search Playbook plan binding must be complete".to_owned());
    }
    if language.is_none() && documents.is_none() {
        return Err("Search Playbook requires --language or --documents producers".to_owned());
    }
    if let Some(requested_workspace_id) = workspace
        && requested_workspace_id != &binding.workspace_id
    {
        return Err(format!(
            "Search Playbook registered workspace binding mismatch: requestedWorkspaceId={requested_workspace_id} boundWorkspaceId={}",
            binding.workspace_id
        ));
    }
    for (block_index, block) in rg.iter().enumerate() {
        let analysis = agent_semantic_shell_parser::analyze_native_rg_argv(block);
        if !analysis.is_admitted() {
            return Err(format!(
                "Search Playbook native rg block is not admitted: blockIndex={block_index} diagnostics={:?}",
                analysis.diagnostics
            ));
        }
    }

    let mut requested = language
        .iter()
        .flat_map(|expression| expression.split('|'))
        .map(|producer| (producer, WorkspaceSearchProducerAxis::Language))
        .chain(
            documents
                .iter()
                .flat_map(|expression| expression.split('|'))
                .map(|producer| (producer, WorkspaceSearchProducerAxis::Document)),
        )
        .collect::<Vec<_>>();
    requested.sort_unstable();
    requested.dedup();
    let mut providers = providers.into_iter().collect::<Vec<_>>();
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    let mut routes = Vec::with_capacity(requested.len());
    let mut admitted_routes = std::collections::BTreeSet::new();
    for (producer, producer_axis) in requested {
        let provider = providers
            .iter()
            .find(|provider| provider.language_id == producer)
            .ok_or_else(|| format!("Search Playbook producer is not registered: {producer}"))?;
        if !provider.search_supported {
            return Err(format!(
                "Search Playbook producer does not support Search: {producer}"
            ));
        }
        if !provider.producer_axes.contains(&producer_axis) {
            return Err(format!(
                "Search Playbook producer is declared on the wrong axis: producer={producer} requestedAxis={producer_axis:?} admittedAxes={:?}",
                provider.producer_axes
            ));
        }
        if !admitted_routes.insert((provider.language_id.clone(), provider.provider_id.clone())) {
            continue;
        }
        routes.push(WorkspaceSearchPlaybookRoute {
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            producer_axis,
            generation_digest: binding.content_generation_digest.clone(),
            extensions: provider.source_extensions.clone(),
        });
    }

    Ok(WorkspaceSearchPlaybookPlan {
        project_id: binding.project_id,
        workspace_id: binding.workspace_id,
        content_generation_digest: binding.content_generation_digest,
        language: language.clone(),
        documents: documents.clone(),
        routes,
        axes: WorkspaceSearchProgressivePlan {
            rg: rg.clone(),
            tantivy: tantivy.clone(),
            syntax: syntax.clone(),
            native_syntax: native_syntax.clone(),
            graph: graph.clone(),
            clause_order: clause_order.clone(),
        },
    })
}
