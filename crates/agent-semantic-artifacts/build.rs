fn main() {
    let config = rust_lang_project_harness::RustHarnessConfig {
        cargo_check_advice_allow_explanation: Some(
            "scope=agent-semantic-artifacts cargo-check advice; owner=artifact identity build gate; finding_category=advisory policy findings; why_safe_now=Phase 1 exposes only typed Merkle identity primitives and keeps DB/search side effects out of this crate while warning and error findings still fail the build; cleanup_trigger=clear any remaining advisory backlog before connecting artifact roots to DB writes"
                .to_string(),
        ),
        ..Default::default()
    };
    rust_lang_project_harness::assert_rust_project_harness_cargo_check_clean_from_env_with_config(
        &config,
    );
}
