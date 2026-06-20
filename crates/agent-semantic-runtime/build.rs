fn main() {
    let config = rust_lang_project_harness::RustHarnessConfig {
        cargo_check_advice_allow_explanation: Some(
            "scope=agent-semantic-runtime cargo-check advice; owner=agent-semantic-runtime build gate; finding_category=advisory policy findings; why_safe_now=runtime state materialization keeps filesystem side effects in a focused crate while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance"
                .to_string(),
        ),
        ..Default::default()
    };
    rust_lang_project_harness::assert_rust_project_harness_cargo_check_clean_from_env_with_config(
        &config,
    );
}
