#[rs_harness::test(
    config = rust_lang_project_harness::default_rust_harness_config()
        .with_cargo_test_advice_allow_explanation(
            "main repository rs_harness attribute smoke keeps harness advice visible",
        ),
    allow_advice,
    should_panic(expected = "main repository rs_harness smoke panic")
)]
fn main_repository_can_use_rs_harness_test_attribute() {
    panic!("main repository rs_harness smoke panic");
}
