// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_config::{
    ProviderPackageKind, ProviderVisibility, ProvidersManifest, ProvidersManifestError,
    load_providers_manifest,
};

#[test]
fn workspace_providers_toml_loads_and_resolves_profiles() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = load_providers_manifest(&workspace_root).expect("load root providers.toml");

    let core = manifest
        .select_packages(Some("core"), &[])
        .expect("core profile");
    assert_eq!(core.len(), 5);
    assert!(core.iter().all(|(_, spec)| {
        spec.kind == ProviderPackageKind::LanguageProvider
            && spec.visibility == ProviderVisibility::Public
    }));

    let analyzers = manifest
        .select_packages(Some("analyzers"), &[])
        .expect("analyzers profile");
    assert_eq!(analyzers.len(), 3);
    assert!(
        analyzers
            .iter()
            .all(|(_, spec)| spec.kind == ProviderPackageKind::Analyzer && spec.optional)
    );
}

#[test]
fn providers_manifest_rejects_invalid_rev_and_path_traversal() {
    let invalid_rev = r#"
schema_version = 1
[packages.asp-rust]
kind = "language-provider"
language = "rust"
repo = "https://github.com/tao3k/asp-rust"
rev = "not-a-sha"
path = "languages/asp-rust"
transport = "grpc"
visibility = "public"
"#;
    assert!(matches!(
        ProvidersManifest::parse_str(invalid_rev),
        Err(ProvidersManifestError::InvalidGitRev(_))
    ));

    let unsafe_path = r#"
schema_version = 1
[packages.asp-rust]
kind = "language-provider"
language = "rust"
repo = "https://github.com/tao3k/asp-rust"
rev = "7a659352c0497823b7492fd1c717f451cfbe0be9"
path = "../escape"
transport = "grpc"
visibility = "public"
"#;
    assert!(matches!(
        ProvidersManifest::parse_str(unsafe_path),
        Err(ProvidersManifestError::InvalidRelativePath { .. })
    ));
}
