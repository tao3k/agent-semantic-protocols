use super::join_capture_projections;

#[test]
fn captures_outside_complete_owners_are_not_semantic_matches() {
    let source = "#![feature(test)]\nfn owned() {}\n";
    let owner_start = source.find("fn owned").expect("function owner") as u64;
    let owner_end = source.len() as u64;
    let owner_projections = vec![agent_semantic_client_db::ProviderSelectorProjection {
        structural_selector: "rust://src/lib.rs#item/function/owned".to_owned(),
        capture_name: "function".to_owned(),
        signature: "fn owned()".to_owned(),
        item_kind: "function".to_owned(),
        item_name: "owned".to_owned(),
        source_byte_start: owner_start,
        source_byte_end: owner_end,
    }];

    let captures = join_capture_projections(
        vec![
            agent_semantic_provider_transport::ProviderSyntaxQueryCapture {
                pattern_index: 0,
                capture_name: "id".to_owned(),
                native_fact_ref: "rust:item:src/lib.rs:2:2:owned".to_owned(),
                source_byte_start: owner_start + 3,
                source_byte_end: owner_start + 8,
            },
        ],
        &owner_projections,
    )
    .expect("join owner-contained captures");

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].item_name, "owned");
    assert_eq!(captures[0].capture_name, "id");
}
