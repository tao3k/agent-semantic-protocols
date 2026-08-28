#[rs_harness::test(
    config = rust_lang_project_harness::default_rust_harness_config()
        .with_cargo_test_advice_allow_explanation(
            "scope=rs_harness_attribute_smoke; owner=agent-semantic-protocol tests; finding_category=attribute-integration-smoke; why_safe_now=the test intentionally panics after verifying the rs_harness attribute wiring; cleanup_trigger=replace when a non-panicking attribute fixture exists",
        ),
    allow_advice,
    should_panic
)]
fn main_repository_can_use_rs_harness_test_attribute() {
    panic!("main repository rs_harness smoke panic");
}
