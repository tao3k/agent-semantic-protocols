use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config, default_rust_harness_config,
};

fn main() {
    let config = default_rust_harness_config().with_cargo_check_advice_allow_explanation(
        "scope=agent-semantic-hook cargo-check advice; owner=agent-semantic-hook build gate; finding_category=advisory policy findings; why_safe_now=semantic-agent-hook keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
    );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
}
