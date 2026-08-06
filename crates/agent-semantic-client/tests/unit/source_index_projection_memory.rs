use super::normalized_item_parser_facts;
use agent_semantic_provider_transport::projection_batch::{
    ProviderProjectedItem, ProviderProjectedItemIdentity,
};

fn fixture_item() -> ProviderProjectedItem {
    ProviderProjectedItem {
        item_id: "item:target".to_owned(),
        owner_id: "owner:src/lib.rs".to_owned(),
        kind: "function".to_owned(),
        name: "target".to_owned(),
        selector: "rust://src/lib.rs#item/function/target".to_owned(),
        source_byte_start: 0,
        source_byte_end: 18,
        identity: ProviderProjectedItemIdentity {
            schema_id: "agent.semantic-protocols.canonical-item-identity".to_owned(),
            schema_version: "1".to_owned(),
            language_id: "rust".to_owned(),
            kind: "function".to_owned(),
            symbol: "target".to_owned(),
            scopes: Vec::new(),
        },
        projections: Vec::new(),
    }
}

#[test]
fn normalized_selector_fact_is_item_local_and_scales_linearly() {
    let item = fixture_item();
    let baseline = normalized_item_parser_facts(&item).expect("encode item-local parser facts");
    let selector_count = 4_096_usize;
    let encoded_bytes = (0..selector_count)
        .map(|_| normalized_item_parser_facts(&item).unwrap().len())
        .sum::<usize>();

    assert_eq!(
        baseline,
        serde_json::to_vec(&item).expect("encode fixture item")
    );
    assert!(baseline.len() < 1_024);
    assert_eq!(
        encoded_bytes,
        selector_count * baseline.len(),
        "selector proof bytes must scale with item facts only"
    );
}
