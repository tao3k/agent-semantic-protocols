use super::{
    asset_name, checksum_name, parse_sha256_checksum, path_segment, provider_release,
    validate_target,
};

#[test]
fn asset_names_are_rev_independent_and_target_selected() {
    let spec = provider_release("julia").expect("julia release spec");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz.sha256"
    );
}

#[test]
fn orgize_release_pin_resolves_provider_binary_asset() {
    let spec = provider_release("org").expect("orgize release spec");
    assert_eq!(spec.provider_id, "orgize");
    assert_eq!(spec.binary, "orgize");
    assert_eq!(spec.release_version, "v0.10.0-alpha.10");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "orgize-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "orgize-aarch64-apple-darwin.tar.gz.sha256"
    );
    assert_eq!(
        super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
            .expect("valid orgize release sha256"),
        Some("bf29b471d01529f5a98364dddac8aa24d5d57f31ba794e3b94e591d8527f9cf8")
    );
}

#[test]
fn pinned_release_hash_requires_a_valid_value_for_each_target() {
    let mut spec = provider_release("org").expect("orgize release spec");
    spec.sha256_by_target.remove("aarch64-apple-darwin");
    let missing = super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
        .expect_err("missing target hash must fail");
    assert!(
        missing.contains("missing pinned release sha256"),
        "{missing}"
    );

    spec.sha256_by_target.insert(
        "aarch64-apple-darwin".to_string(),
        "NOT-A-SHA256".to_string(),
    );
    let invalid = super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
        .expect_err("invalid target hash must fail");
    assert!(
        invalid.contains("invalid pinned release sha256"),
        "{invalid}"
    );
}

#[test]
fn parse_checksum_accepts_common_sha256_formats() {
    assert_eq!(
        parse_sha256_checksum(
            "ABCDEFabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123  file.tar.gz\n"
        )
        .as_deref(),
        Some("abcdefabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123")
    );
}

#[test]
fn rev_path_segment_is_filesystem_safe() {
    assert_eq!(
        path_segment("refs/tags/v1.2.3+build"),
        "refs_tags_v1.2.3_build"
    );
}

#[test]
fn unsupported_apple_intel_target_is_rejected() {
    let spec = provider_release("rust").expect("rust release spec");
    let error = validate_target(&spec, "x86_64-apple-darwin").expect_err("unsupported target");
    assert!(error.contains("unsupported target `x86_64-apple-darwin`"));
    assert!(error.contains("aarch64-apple-darwin"));
}
