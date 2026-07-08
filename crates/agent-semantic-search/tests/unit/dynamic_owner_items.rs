use std::fs;

use agent_semantic_search::{
    DynamicOwnerItem, DynamicOwnerItemsRequest, DynamicOwnerPath, DynamicOwnerQuery,
    DynamicSearchLanguage, DynamicSearchRoots, render_dynamic_owner_items_frontier,
};

#[test]
fn owner_local_phrase_hit_attributes_to_parser_item() {
    let root = std::env::temp_dir().join(format!("asp-dynamic-owner-items-{}", std::process::id()));
    let owner = root.join("src/lib.rs");
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("create fixture dir");
    fs::write(
        &owner,
        "pub fn agent_session_artifact_activity() {\n    let heartbeat = true;\n}\n",
    )
    .expect("write fixture owner");

    let items = [DynamicOwnerItem::new(
        "agent_session_artifact_activity",
        "function",
        1,
        3,
    )];
    let output = render_dynamic_owner_items_frontier(DynamicOwnerItemsRequest {
        language: DynamicSearchLanguage::new("rust"),
        roots: DynamicSearchRoots::new(&root, &root),
        owner: DynamicOwnerPath::new(std::path::Path::new("src/lib.rs")),
        query: DynamicOwnerQuery::new("heartbeat"),
        items: &items,
    });

    assert!(
        output.contains(
            "structuralSelector=rust://src/lib.rs#item/function/agent_session_artifact_activity"
        ),
        "{output}"
    );
    assert!(
        output.contains("reason=owner-local-source-attribution"),
        "{output}"
    );
    assert!(!output.contains("item=0"), "{output}");

    fs::remove_dir_all(root).expect("remove fixture root");
}
