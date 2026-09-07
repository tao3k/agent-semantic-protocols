// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Agent-prioritized progressive plan for the public Search Playbook.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProvider {
    pub language_id: String,
    pub provider_id: String,
    pub source_extensions: Vec<String>,
    pub search_supported: bool,
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
    pub generation_digest: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProgressivePlan {
    pub fd: Vec<Vec<String>>,
    pub rg: Vec<Vec<String>>,
    pub tantivy: Vec<Vec<String>>,
    pub syntax: Vec<crate::ProducerNativeBlock>,
    pub native_syntax: Vec<String>,
    pub graph: Vec<crate::GraphNativeBlock>,
    pub clause_order: Vec<crate::SearchPlaybookClauseRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchWarmWork {
    pub filesystem_read_count: u64,
    pub provider_process_count: u64,
    pub socket_discovery_count: u64,
}

/// Internal scheduler input. This type is deliberately not serializable and
/// can never be returned as a public Search terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchPlaybookPlan {
    pub project_id: String,
    pub workspace_id: String,
    pub content_generation_digest: String,
    pub languages: Option<String>,
    pub documents: Option<String>,
    pub routes: Vec<WorkspaceSearchPlaybookRoute>,
    pub axes: WorkspaceSearchProgressivePlan,
    pub work: WorkspaceSearchWarmWork,
}

pub fn build_workspace_search_playbook_plan(
    request: &crate::ProgressiveSearchPlaybookRequest,
    binding: WorkspaceSearchPlanBinding,
    providers: impl IntoIterator<Item = WorkspaceSearchProvider>,
) -> Result<WorkspaceSearchPlaybookPlan, String> {
    let crate::ProgressiveSearchPlaybookRequest {
        languages,
        documents,
        workspace: _,
        fd,
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

    let mut requested = languages
        .iter()
        .chain(documents.iter())
        .flat_map(|expression| expression.split('|'))
        .collect::<Vec<_>>();
    requested.sort_unstable();
    requested.dedup();
    if requested.is_empty() {
        return Err("Search Playbook execution requires a producer".to_owned());
    }

    let mut providers = providers.into_iter().collect::<Vec<_>>();
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    let mut routes = Vec::with_capacity(requested.len());
    for producer in requested {
        let provider = providers
            .iter()
            .find(|provider| provider.language_id == producer)
            .ok_or_else(|| format!("Search Playbook producer is not registered: {producer}"))?;
        if !provider.search_supported {
            return Err(format!(
                "Search Playbook producer does not support Search: {producer}"
            ));
        }
        routes.push(WorkspaceSearchPlaybookRoute {
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            generation_digest: binding.content_generation_digest.clone(),
            extensions: provider.source_extensions.clone(),
        });
    }

    Ok(WorkspaceSearchPlaybookPlan {
        project_id: binding.project_id,
        workspace_id: binding.workspace_id,
        content_generation_digest: binding.content_generation_digest,
        languages: languages.clone(),
        documents: documents.clone(),
        routes,
        axes: WorkspaceSearchProgressivePlan {
            fd: fd.clone(),
            rg: rg.clone(),
            tantivy: tantivy.clone(),
            syntax: syntax.clone(),
            native_syntax: native_syntax.clone(),
            graph: graph.clone(),
            clause_order: clause_order.clone(),
        },
        work: WorkspaceSearchWarmWork {
            filesystem_read_count: 0,
            provider_process_count: 0,
            socket_discovery_count: 0,
        },
    })
}
