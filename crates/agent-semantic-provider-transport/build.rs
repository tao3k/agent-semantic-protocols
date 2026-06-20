use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config,
    assert_rust_project_harness_verification_from_env_with_config, default_rust_harness_config,
};

fn main() {
    let config = default_rust_harness_config()
        .with_cargo_check_advice_allow_explanation(
            "scope=agent-semantic-provider-transport cargo-check advice; owner=agent-semantic-provider-transport build gate; finding_category=advisory policy findings; why_safe_now=provider transport keeps process orchestration advice visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        )
        .with_criterion_performance_verification()
        .with_latency_sensitive_performance_owner(
            "src/capture.rs",
            "provider stdout/stderr capture is a hot path for every native provider command",
        )
        .with_latency_sensitive_performance_owner(
            "src/transport.rs",
            "provider process orchestration controls command latency and timeout behavior",
        )
        .with_latency_sensitive_performance_owner(
            "src/byte_text.rs",
            "byte-text projection is reused by compact search and provider output rendering",
        )
        .with_availability_stability_owner(
            "src/transport.rs",
            "provider process orchestration must keep timeout, kill, and broken-pipe behavior deterministic",
        );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
    assert_rust_project_harness_verification_from_env_with_config(&config, "provider transport");
}
