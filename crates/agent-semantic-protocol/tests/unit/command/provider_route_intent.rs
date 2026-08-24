use super::{runtime_owner_intent, runtime_query_intent, runtime_search_intent};

#[test]
fn route_intents_are_semantic_and_do_not_forward_argv() {
    let search = runtime_search_intent(
        &["search", "pipe", "tokio stream", "--workspace", "."].map(str::to_owned),
    )
    .expect("search intent");
    assert_eq!(
        search["schemaId"],
        "agent.semantic-protocols.asp-client-search-request"
    );
    assert_eq!(search["schemaVersion"], "1");
    assert_eq!(search["query"], "tokio stream");
    assert_eq!(search["operation"], "pipe");
    assert!(search.get("argv").is_none());

    let owner = runtime_owner_intent(
        &[
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "Runtime",
        ]
        .map(str::to_owned),
    )
    .expect("owner intent");
    assert_eq!(
        owner["schemaId"],
        "agent.semantic-protocols.asp-client-owner-search-request"
    );
    assert_eq!(owner["ownerPath"], "src/lib.rs");
    assert_eq!(owner["query"], "Runtime");
    assert!(owner.get("argv").is_none());

    let query = runtime_query_intent(
        &[
            "query",
            "--selector",
            "src/lib.rs:1:4",
            "--projection",
            "source",
        ]
        .map(str::to_owned),
    )
    .expect("query intent");
    assert_eq!(
        query["schemaId"],
        "agent.semantic-protocols.asp-client-exact-query-request"
    );
    assert_eq!(query["selector"], "src/lib.rs:1:4");
    assert_eq!(query["projection"], "source");
    assert!(query.get("argv").is_none());

    let positional_query = runtime_query_intent(
        &["query", "src/lib.rs:1:4", "--projection", "source"].map(str::to_owned),
    )
    .expect("positional query intent");
    assert_eq!(
        positional_query, query,
        "accepted exact-query command shapes must normalize to one Runtime intent"
    );
}
