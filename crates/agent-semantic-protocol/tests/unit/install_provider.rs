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
