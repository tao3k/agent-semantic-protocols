fn main() {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut config = rust_lang_project_harness::rust_harness_config_for_project(project_root);
    config.verification_policy.profile_hints.push(
        rust_lang_project_harness::RustVerificationProfileHint::new(
            std::path::PathBuf::from("src/lib.rs"),
            [rust_lang_project_harness::RustOwnerResponsibility::AvailabilityCritical],
        )
        .with_task_kinds([rust_lang_project_harness::RustVerificationTaskKind::Stability])
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
    let policy_bytes = serde_json::to_vec(policy.config())
        .expect("serialize ASP Rust build-support harness policy");
    let policy_digest = format!("blake3-256:{}", blake3::hash(&policy_bytes).to_hex());
    let out_dir = std::env::var_os("OUT_DIR")
        .map(std::path::PathBuf::from)
        .expect("Cargo OUT_DIR is required; implicit cache fallback is forbidden");
    let authority = rust_lang_project_harness::RustProjectHarnessBuildGateAuthority::new(
        out_dir.join("rust-project-harness-self-policy-cache"),
        policy_digest,
    )
    .expect("construct ASP Rust build-support harness authority");
    rust_lang_project_harness::assert_rust_project_harness_downstream_policy_with_authority(
        project_root,
        &policy,
        &authority,
    );
}
