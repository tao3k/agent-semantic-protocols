use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config, default_rust_harness_config,
};

fn main() {
    let config = default_rust_harness_config()
        .with_cargo_check_advice_allow_explanation(
            "scope=agent-semantic-search cargo-check advice; owner=agent-semantic-search build gate; finding_category=advisory policy findings; why_safe_now=search crate keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        )
        .with_latency_sensitive_performance_owner(
            "src/dynamic_overlay",
            "dynamic overlay search/query routing sits on repeated agent search hot paths",
        )
        .with_latency_sensitive_performance_owner(
            "src/turso_overlay_search.rs",
            "Turso-backed overlay search must remain subsecond under dynamic agent query fanout",
        );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
}
