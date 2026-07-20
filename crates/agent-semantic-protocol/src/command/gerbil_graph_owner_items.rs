use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_search::{
    GraphOwnerItemRenderRequest, GraphOwnerItemRouteRequest, rank_graph_owner_items,
    render_graph_owner_item_frontier,
};

const OWNER_ITEM_LIMIT: u32 = 32;

/// Render the Gerbil owner-item route exclusively from its persisted EvidenceGraph.
pub(super) fn render_gerbil_graph_owner_items(
    project_root: &Path,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    owner: &Path,
    query: &str,
    query_pack_descriptor: agent_semantic_search::SearchPipeQueryPackDescriptor<'_>,
) -> Result<String, String> {
    let owner = owner
        .to_str()
        .ok_or_else(|| "Gerbil owner path is not valid UTF-8".to_string())?;
    let language_id = LanguageId::from("gerbil-scheme");
    let read_model = ClientDbEngine::lookup_graph_owner_read_model_from_project(
        project_root,
        source_snapshot,
        owner,
        Some(&language_id),
        OWNER_ITEM_LIMIT,
    )?;
    if read_model.artifact_evidence.authority_state
        != agent_semantic_content_identity::DerivedArtifactAuthorityState::Current
        || !read_model.owner_present
    {
        return Ok(render_projection_import_required(owner, query));
    }

    let clauses =
        super::search_pipe_query_pack::query_clauses("gerbil-scheme", query, query_pack_descriptor);
    let query_terms = super::search_pipe_query_pack::unique_query_terms(&clauses)
        .into_iter()
        .map(|term| term.raw)
        .collect::<Vec<_>>();
    let route = rank_graph_owner_items(GraphOwnerItemRouteRequest {
        owner_path: owner,
        query_terms: &query_terms,
        nodes: &read_model.selector_nodes,
    });
    let resolved_snapshot = &read_model.artifact_evidence.source_snapshot;
    let graph_artifact_digest = read_model
        .artifact_evidence
        .resolved_artifact_digest
        .as_deref()
        .ok_or_else(|| "current graph artifact omitted its content digest".to_owned())?;
    Ok(render_graph_owner_item_frontier(
        GraphOwnerItemRenderRequest {
            language_id: "gerbil-scheme",
            owner_path: owner,
            query,
            route: &route,
            snapshot_root: &resolved_snapshot.root_digest,
            graph_artifact_digest,
        },
    ))
}

fn render_projection_import_required(owner: &str, query: &str) -> String {
    format!(
        "[search-owner] q={query} owner={owner} selector=items alg=graph-turbo-owner-items\nstate=projection-cold-required providerProcessCount=0\nnextAction=projection-import-required\nnextCommand=asp gerbil-scheme projection import --owner {} --workspace .\nentries=owner-query(O,Q=>projection-lifecycle)\n",
        shell_quote(owner)
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
