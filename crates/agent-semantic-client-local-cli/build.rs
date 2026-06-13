use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config,
    assert_rust_project_harness_verification_from_env_with_config, default_rust_harness_config,
};

fn main() {
    let config = default_rust_harness_config()
        .with_cargo_check_advice_allow_explanation(
            "agent-semantic-client-local-cli keeps advisory findings visible while the build gate blocks warning and error policy drift",
        )
        .with_criterion_performance_verification()
        .with_latency_sensitive_performance_owner(
            "src/backend.rs",
            "local native backend fans out provider commands and aggregates captured Bytes output",
        )
        .with_availability_stability_owner(
            "src/backend.rs",
            "local native backend must preserve deterministic provider routing and error handling under repeated execution",
        );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
    assert_rust_project_harness_verification_from_env_with_config(&config, "local native backend");
}
