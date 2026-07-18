use std::path::PathBuf;

use super::{
    Candidate, QueryWrapperQuality, QueryWrapperSurface, query_wrapper_action_nodes,
    render_query_wrapper_next_classes,
};

fn source_index_candidate(path: &str, symbol: &str) -> Candidate {
    Candidate {
        path: path.to_string(),
        line: 1,
        end_line: 1,
        symbol: symbol.to_string(),
        selector: None,
        text: symbol.to_string(),
        source: "source-index".to_string(),
        confidence: "db-engine".to_string(),
    }
}

fn cohesive_quality() -> QueryWrapperQuality {
    QueryWrapperQuality {
        query_pack_quality: "medium".to_string(),
        scope_quality: "low".to_string(),
        package_cohesion: "high".to_string(),
        packages: vec!["agent-semantic-protocol".to_string()],
        risks: vec!["broad-scope".to_string()],
        noise: Vec::new(),
        allow_query_selector: true,
        clause_coverages: Vec::new(),
    }
}

#[test]
fn rg_source_index_owner_hit_routes_directly_to_owner_items() {
    let symbol = "codex_fs_read_file_decision";
    let candidates = vec![source_index_candidate(
        "crates/agent-semantic-protocol/src/command/hook_runtime_source_access.rs",
        symbol,
    )];
    let queries = vec![symbol.to_string()];
    let terms = queries.clone();
    let actions = query_wrapper_action_nodes(
        QueryWrapperSurface::Rg,
        &[PathBuf::from(".")],
        &queries,
        &terms,
        &candidates,
        &cohesive_quality(),
    );

    assert_eq!(actions[0].kind, "owner-items");
    assert_eq!(actions[1].kind, "fd-query");
    assert_eq!(
        render_query_wrapper_next_classes(
            QueryWrapperSurface::Rg,
            &[PathBuf::from(".")],
            &queries,
            &terms,
            &candidates,
            &cohesive_quality(),
        ),
        "owner-items,fd-query"
    );
}
