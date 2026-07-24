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

#[test]
fn owner_no_match_emits_semantic_frontier_without_legacy_wrapper_command() {
    let root =
        std::env::temp_dir().join(format!("asp-dynamic-owner-no-match-{}", std::process::id()));
    let owner = root.join("src/lib.rs");
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("create fixture dir");
    fs::write(&owner, "pub fn existing_item() {}\n").expect("write fixture owner");
    let items = [DynamicOwnerItem::new("existing_item", "function", 1, 1)];

    let output = render_dynamic_owner_items_frontier(DynamicOwnerItemsRequest {
        language: DynamicSearchLanguage::new("rust"),
        roots: DynamicSearchRoots::new(&root, &root),
        owner: DynamicOwnerPath::new(std::path::Path::new("src/lib.rs")),
        query: DynamicOwnerQuery::new("missing_symbol"),
        items: &items,
    });

    assert_eq!(output.matches("[search-owner]").count(), 1, "{output}");
    assert!(output.contains("reason=no-owner-item-match"), "{output}");
    assert!(output.contains("recommendedNext=revise-query"), "{output}");
    assert!(output.contains("actionFrontier=revise-query"), "{output}");
    assert!(!output.contains("asp rg"), "{output}");
    assert!(!output.contains("asp fd"), "{output}");
    fs::remove_dir_all(root).expect("remove fixture root");
}

#[test]
fn owner_local_source_hit_emits_only_the_most_specific_ast_item() {
    let root = std::env::temp_dir().join(format!(
        "asp-dynamic-owner-item-minimal-cut-{}",
        std::process::id()
    ));
    let owner = root.join("src/lib.rs");
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("create fixture dir");
    fs::write(
        &owner,
        "impl ClientDbEngine {\n    fn persist_source_index_read_model(&self) {\n        let persisted = true;\n    }\n}\n",
    )
    .expect("write fixture owner");

    let items = [
        DynamicOwnerItem::new("ClientDbEngine", "impl", 1, 5),
        DynamicOwnerItem::new("persist_source_index_read_model", "method", 2, 4),
    ];
    let output = render_dynamic_owner_items_frontier(DynamicOwnerItemsRequest {
        language: DynamicSearchLanguage::new("rust"),
        roots: DynamicSearchRoots::new(&root, &root),
        owner: DynamicOwnerPath::new(std::path::Path::new("src/lib.rs")),
        query: DynamicOwnerQuery::new("persisted"),
        items: &items,
    });

    assert!(
        output.contains(
            "structuralSelector=rust://src/lib.rs#item/method/persist_source_index_read_model"
        ),
        "{output}"
    );
    assert!(
        !output.contains("structuralSelector=rust://src/lib.rs#item/impl/ClientDbEngine"),
        "{output}"
    );

    fs::remove_dir_all(root).expect("remove fixture root");
}

#[test]
fn owner_local_alternatives_follow_the_shared_query_grammar() {
    let root = std::env::temp_dir().join(format!(
        "asp-dynamic-owner-items-alternatives-{}",
        std::process::id()
    ));
    let owner = root.join("src/lib.rs");
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("create fixture dir");
    fs::write(
        &owner,
        "pub fn run_language_command() {\n    activate_provider();\n}\n",
    )
    .expect("write fixture owner");

    let items = [DynamicOwnerItem::new(
        "run_language_command",
        "function",
        1,
        3,
    )];
    let output = render_dynamic_owner_items_frontier(DynamicOwnerItemsRequest {
        language: DynamicSearchLanguage::new("rust"),
        roots: DynamicSearchRoots::new(&root, &root),
        owner: DynamicOwnerPath::new(std::path::Path::new("src/lib.rs")),
        query: DynamicOwnerQuery::new("missing_term|activate_provider|preflight"),
        items: &items,
    });

    assert!(
        output.contains("item/function/run_language_command"),
        "{output}"
    );
    assert!(
        output.contains("reason=owner-local-source-attribution"),
        "{output}"
    );

    fs::remove_dir_all(root).expect("remove fixture root");
}
