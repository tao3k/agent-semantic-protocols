use crate::dynamic_overlay::DynamicOverlayDocument;
use crate::dynamic_overlay::DynamicOverlayNamespace;
use crate::dynamic_overlay::DynamicOverlayQuery;
use crate::dynamic_overlay::DynamicOverlaySearchBackend;
use crate::dynamic_overlay::InMemoryDynamicOverlaySearch;

fn document(entity_id: &str, text: &str) -> DynamicOverlayDocument {
    DynamicOverlayDocument {
        owner_path: format!("src/{entity_id}.rs"),
        entity_id: entity_id.to_owned(),
        selector: format!("rust://src/{entity_id}.rs#item/function/{entity_id}"),
        kind: "function".to_owned(),
        name: entity_id.to_owned(),
        signature: None,
        display_range: None,
        source_hash: format!("digest-{entity_id}"),
        search_text: text.to_owned(),
    }
}

#[test]
fn resident_lexical_postings_bound_work_to_matching_owners() {
    let namespace =
        DynamicOverlayNamespace::new("project", "workspace", "worktree", "session", "generation");
    let mut search = InMemoryDynamicOverlaySearch::default();
    let mut documents = (0..1_024)
        .map(|index| document(&format!("ordinary_{index}"), "common lexical owner"))
        .collect::<Vec<_>>();
    documents.push(document("rare_owner", "rareterm lexical owner"));
    search.upsert_documents(namespace.clone(), documents);

    let indexed_candidates = search.candidate_count_for_query(&namespace, "rareterm");
    assert_eq!(indexed_candidates, 1);
    let full_scan_candidates = 1_025usize;
    assert!(full_scan_candidates / indexed_candidates >= 1_000);
    println!(
        "{{\"schemaId\":\"agent.semantic-protocols.lexical-postings-work-reduction-receipt\",\"schemaVersion\":\"1\",\"fullScanCandidates\":{full_scan_candidates},\"indexedCandidates\":{indexed_candidates},\"reductionFactor\":{}}}",
        full_scan_candidates / indexed_candidates
    );
    let hits = search.search(&namespace, &DynamicOverlayQuery::new("rareterm"));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document.entity_id, "rare_owner");
}

#[test]
fn overlay_upsert_removes_stale_term_postings() {
    let namespace =
        DynamicOverlayNamespace::new("project", "workspace", "worktree", "session", "generation");
    let mut search = InMemoryDynamicOverlaySearch::default();
    search.upsert_documents(namespace.clone(), vec![document("owner", "obsolete_term")]);
    assert_eq!(
        search.candidate_count_for_query(&namespace, "obsolete_term"),
        1
    );

    search.upsert_documents(
        namespace.clone(),
        vec![document("owner", "replacement_term")],
    );

    assert_eq!(
        search.candidate_count_for_query(&namespace, "obsolete_term"),
        0
    );
    assert_eq!(
        search.candidate_count_for_query(&namespace, "replacement_term"),
        1
    );
}

#[test]
fn merkle_generation_namespaces_cannot_observe_each_others_postings() {
    let root_a = DynamicOverlayNamespace::new(
        "project",
        "workspace",
        "worktree",
        "provider-digest",
        "source-root-a",
    );
    let root_b = DynamicOverlayNamespace::new(
        "project",
        "workspace",
        "worktree",
        "provider-digest",
        "source-root-b",
    );
    let mut search = InMemoryDynamicOverlaySearch::default();
    search.upsert_documents(root_a.clone(), vec![document("owner", "root_a_term")]);
    search.upsert_documents(root_b.clone(), vec![document("owner", "root_b_term")]);

    assert_eq!(search.candidate_count_for_query(&root_a, "root_a_term"), 1);
    assert_eq!(search.candidate_count_for_query(&root_a, "root_b_term"), 0);
    assert_eq!(search.candidate_count_for_query(&root_b, "root_a_term"), 0);
    assert_eq!(search.candidate_count_for_query(&root_b, "root_b_term"), 1);
}
