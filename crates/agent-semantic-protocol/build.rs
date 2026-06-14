use rust_lang_project_harness::{
    assert_rust_project_harness_cargo_check_clean_from_env_with_config, default_rust_harness_config,
};

fn main() {
    println!("cargo:rerun-if-changed=../../SKILL.org");
    println!("cargo:rerun-if-changed=../../SKILL.contract.org");

    let config = default_rust_harness_config().with_cargo_check_advice_allow_explanation(
        "semantic-agent-protocol keeps advisory findings visible while the build gate blocks warning and error policy drift",
    );
    assert_rust_project_harness_cargo_check_clean_from_env_with_config(&config);
}
