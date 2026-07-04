use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config,
    assert_rust_project_harness_verification_from_env_with_config, default_rust_harness_config,
};

fn main() {
    let config = default_rust_harness_config()
        .with_cargo_check_advice_allow_explanation(
            "scope=agent-semantic-client-db cargo-check advice; owner=agent-semantic-client-db build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-db keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        )
        .with_criterion_performance_verification()
        .with_latency_sensitive_performance_owner(
            "src/engine/facade.rs",
            "DB Engine facade routes provider replay hot paths through active Turso adapters",
        )
        .with_latency_sensitive_performance_owner(
            "src/engine/turso_cache.rs",
            "Turso cache generation lookup and invalidation sit on repeated agent search replay paths",
        )
        .with_availability_stability_owner(
            "src/engine/turso.rs",
            "Turso bootstrap schema and transaction boundaries must remain stable under repeated agent writeback and replay",
        );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
    assert_rust_project_harness_verification_from_env_with_config(&config, "client db");
}
