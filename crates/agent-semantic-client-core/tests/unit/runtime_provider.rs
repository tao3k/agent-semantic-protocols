// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{LanguageId, ProviderId, RuntimeProvider, RuntimeProviderProjection};

use super::test_support::runtime_provider;

#[test]
fn projection_evidence_binds_runtime_registration_and_existing_scope() {
    let root = temp_root("projection-evidence");
    std::fs::create_dir_all(root.join("crates/core/src")).expect("create source dir");
    std::fs::write(
        root.join("crates/core/Cargo.toml"),
        "[package]\nname='core'\n",
    )
    .expect("write config file");
    let mut provider = runtime_provider();
    provider.package_roots = vec!["crates/core".to_string()];
    provider.config_files = vec!["crates/core/Cargo.toml".to_string()];
    provider.source_extensions = vec!["rs".to_string()];
    let projection = RuntimeProviderProjection {
        authority_ref: "runtime-provider-register:test-generation".to_string(),
        providers: vec![provider],
    };

    let evidence = projection.evidence(&root);

    assert!(evidence.fingerprint.contains("authority=runtime-provider-register:"));
    assert!(evidence.fingerprint.contains("registrationDigest=sha256:test"));
    assert!(evidence.fingerprint.contains("language=rust"));
    assert!(evidence.fingerprint.contains("provider=asp-rust"));
    assert!(!evidence.fingerprint.contains("manifest"));
    assert!(evidence.scope_dirs.contains("crates/core"));
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn projection_fingerprint_binds_provider_descriptors() {
    let root = temp_root("projection-descriptor-fingerprint");
    let baseline = evidence_fingerprint(&root, runtime_provider());
    let mut mutations = Vec::new();

    let mut registration = runtime_provider();
    registration.registration_digest = "sha256:changed".to_string();
    mutations.push(registration);
    let mut language = runtime_provider();
    language.language_id = LanguageId::from("rust-variant");
    mutations.push(language);
    let mut provider_id = runtime_provider();
    provider_id.provider_id = ProviderId::from("asp-rust-variant");
    mutations.push(provider_id);
    let mut search = runtime_provider();
    search.search_capabilities.owner_items = !search.search_capabilities.owner_items;
    mutations.push(search);
    let mut query_pack = runtime_provider();
    query_pack.query_pack_descriptor.descriptor_id = "rust.changed".to_string();
    mutations.push(query_pack);

    for mutated in mutations {
        assert_ne!(evidence_fingerprint(&root, mutated), baseline);
    }
    std::fs::remove_dir_all(root).expect("remove temp root");
}

fn evidence_fingerprint(root: &std::path::Path, provider: RuntimeProvider) -> String {
    RuntimeProviderProjection {
        authority_ref: "runtime-provider-register:test-generation".to_string(),
        providers: vec![provider],
    }
    .evidence(root)
    .fingerprint
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-core-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
