use super::{
    is_search_owner_items_query, parse_search_owner_items_query_args,
};

#[test]
fn owner_parser_consumes_the_items_projection_before_options() {
    let args = [
        "search",
        "owner",
        "src/lib.rs",
        "items",
        "--query",
        "Service|Repository",
        "--view",
        "seeds",
    ]
    .map(str::to_owned);

    let parsed = parse_search_owner_items_query_args(&args).expect("owner args");
    assert_eq!(parsed.owner, std::path::PathBuf::from("src/lib.rs"));
    assert_eq!(parsed.query, "Service|Repository");
    assert_eq!(parsed.view, "seeds");
}

#[test]
fn pipe_command_is_never_classified_as_owner_search() {
    let args = ["search", "pipe", "kind|owner|tests", "--view", "seeds"].map(str::to_owned);
    assert!(!is_search_owner_items_query(&args));
}
