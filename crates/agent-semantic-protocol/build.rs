use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config, default_rust_harness_config,
};

fn main() {
    println!("cargo:rerun-if-changed=../../languages/org/contracts/asp.skill.v1.org");

    let config = default_rust_harness_config().with_cargo_check_advice_allow_explanation(
        "scope=agent-semantic-protocol cargo-check advice; owner=agent-semantic-protocol build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-protocol keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
    );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
}
