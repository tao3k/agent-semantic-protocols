use super::source_query_keys;

#[test]
fn query_keys_include_snake_and_kebab_components() {
    let keys = source_query_keys(
        "src/report-chain.rs",
        "fn topology_membership_report_chain_request_policy() {}",
    );

    for expected in [
        "topology",
        "membership",
        "report",
        "chain",
        "request",
        "policy",
    ] {
        assert!(keys.iter().any(|key| key == expected), "missing {expected}");
    }
}
