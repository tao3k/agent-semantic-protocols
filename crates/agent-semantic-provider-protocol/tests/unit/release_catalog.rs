// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::parse_provider_release_catalog;

fn release_catalog(extra: &str) -> String {
    format!(
        r#"schemaId = "agent.semantic-protocols.provider-release-catalog"
schemaVersion = "1"

[releases.rust]
languageId = "rust"
providerId = "asp-rust"
binary = "asp-rust"
repo = "tao3k/rust-lang-project-harness"
version = "v1.0.0"
downloadBaseUrl = "https://example.invalid/releases/v1.0.0"
supportedTargets = ["aarch64-apple-darwin"]
sha256ByTarget = {{ "aarch64-apple-darwin" = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" }}
{extra}
"#
    )
}

#[test]
fn release_catalog_requires_one_immutable_checksum_per_supported_target() {
    let missing = release_catalog("").replace(
        "sha256ByTarget = { \"aarch64-apple-darwin\" = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\" }\n",
        "",
    );
    let error = parse_provider_release_catalog(&missing)
        .expect_err("release without target checksum must fail closed");
    assert!(error.contains("checksum coverage mismatch"), "{error}");

    let extra = release_catalog("").replace(
        "sha256ByTarget = { \"aarch64-apple-darwin\" = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\" }",
        "sha256ByTarget = { \"aarch64-apple-darwin\" = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"x86_64-unknown-linux-gnu\" = \"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\" }",
    );
    let error = parse_provider_release_catalog(&extra)
        .expect_err("checksum for unsupported target must fail closed");
    assert!(error.contains("checksum coverage mismatch"), "{error}");
}

#[test]
fn release_catalog_contains_only_immutable_release_inputs() {
    let catalog = parse_provider_release_catalog(&release_catalog(""))
        .expect("parse immutable release catalog");
    let rust = &catalog.releases["rust"];
    assert_eq!(rust.provider_id, "asp-rust");
    assert_eq!(rust.binary, "asp-rust");
    assert_eq!(rust.release_version, "v1.0.0");
}

#[test]
fn release_catalog_rejects_development_source_fields() {
    let error = parse_provider_release_catalog(&release_catalog(
        "sourceRoot = \"languages/asp-rust\"\nworkspaceInstall = \"provider/install.json\"",
    ))
    .expect_err("release catalog must reject source build authority");
    assert!(error.contains("unknown field"), "{error}");
}

#[test]
fn release_catalog_rejects_language_and_provider_identity_drift() {
    let source = release_catalog("").replace("languageId = \"rust\"", "languageId = \"python\"");
    let error = parse_provider_release_catalog(&source).expect_err("identity drift must fail");
    assert!(error.contains("language key drift"), "{error}");
}
