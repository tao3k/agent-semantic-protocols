#[path = "../../../src/command/dispatch_agent_session_policy.rs"]
mod dispatch_agent_session_policy;

use dispatch_agent_session_policy::is_agent_session_direct_inventory_or_fetch_command;

fn args(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_string()).collect()
}

#[test]
fn exact_selector_code_query_is_direct_fetch() {
    let command = args(&[
        "rust",
        "query",
        "--selector",
        "src/lib.rs:10:20",
        "--workspace",
        ".",
        "--code",
    ]);

    assert!(is_agent_session_direct_inventory_or_fetch_command(&command));
}

#[test]
fn exact_selector_content_query_is_direct_fetch() {
    let command = args(&[
        "md",
        "query",
        "--selector",
        "README.md",
        "--workspace",
        ".",
        "--content",
    ]);

    assert!(is_agent_session_direct_inventory_or_fetch_command(&command));
}

#[test]
fn exact_selector_metadata_query_is_direct_inventory() {
    let command = args(&[
        "org",
        "query",
        "--selector",
        "README.org",
        "--workspace",
        ".",
        "--view",
        "metadata",
    ]);

    assert!(is_agent_session_direct_inventory_or_fetch_command(&command));
}

#[test]
fn broad_search_is_not_direct_inventory_or_fetch() {
    let command = args(&["rust", "search", "lexical", "query/search command denied"]);

    assert!(!is_agent_session_direct_inventory_or_fetch_command(
        &command
    ));
}

#[test]
fn term_query_without_selector_is_not_direct_inventory_or_fetch() {
    let command = args(&["rust", "query", "--term", "query/search command denied"]);

    assert!(!is_agent_session_direct_inventory_or_fetch_command(
        &command
    ));
}

#[test]
fn hook_direct_source_read_is_not_main_direct_fetch() {
    let command = args(&[
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "src/lib.rs:10:20",
        "--code",
    ]);

    assert!(!is_agent_session_direct_inventory_or_fetch_command(
        &command
    ));
}
