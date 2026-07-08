fn main() {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut config = rust_lang_project_harness::rust_harness_config_for_project(project_root);
    config.verification_policy.profile_hints.push(
        rust_lang_project_harness::RustVerificationProfileHint::new(
            std::path::PathBuf::from("src/lib.rs"),
            [
                rust_lang_project_harness::RustOwnerResponsibility::LatencySensitive,
                rust_lang_project_harness::RustOwnerResponsibility::AvailabilityCritical,
            ],
        )
        .with_task_kinds([
            rust_lang_project_harness::RustVerificationTaskKind::Performance,
            rust_lang_project_harness::RustVerificationTaskKind::Stability,
        ])
        .with_rationale(
            "asp-rust-project-harness-policy owns build-support evidence graph policy for ASP",
        ),
    );
    if config.verification_policy.stability_picture.is_none() {
        config.verification_policy.stability_picture =
            Some(rust_lang_project_harness::RustVerificationStabilityPictureConfig::default());
    }
    let policy = rust_lang_project_harness::RustProjectHarnessDownstreamPolicy::new(
        "asp-rust-project-harness-policy",
        config,
    );
    rust_lang_project_harness::assert_rust_project_harness_downstream_policy_from_env(&policy);
}
