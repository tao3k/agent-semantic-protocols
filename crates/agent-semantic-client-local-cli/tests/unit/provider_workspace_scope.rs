use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    LanguageId, ProviderExecution, ProviderId, ProviderRegistrySnapshot, ResolvedProvider,
};

use super::{
    PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID, ProviderWorkspaceScope, ProviderWorkspaceScopeFile,
    ProviderWorkspaceScopePacket, collect_provider_source_scope_files,
    provider_workspace_scope_files_from_packet, provider_workspace_scope_from_stdout,
};

#[test]
fn workspace_scope_packet_defaults_language_and_provider_from_provider() {
    let provider = provider();
    let stdout = br#"{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","files":[{"path":"src/lib.rs"}]}"#;

    let scope =
        provider_workspace_scope_from_stdout(stdout, &provider).expect("parse workspace scope");

    let ProviderWorkspaceScope::Supported(packet) = scope else {
        panic!("workspace-scope packet should be supported");
    };
    assert_eq!(packet.language_id.as_str(), "rust");
    assert_eq!(packet.provider_id.as_str(), "rs-harness");
    assert_eq!(packet.files.len(), 1);
    assert_eq!(packet.files[0].path, "src/lib.rs");
    assert_eq!(packet.files[0].language_id.as_str(), "rust");
    assert_eq!(packet.files[0].provider_id.as_str(), "rs-harness");
}

#[test]
fn workspace_scope_packet_preserves_file_language_and_provider_overrides() {
    let provider = provider();
    let stdout = br#"{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","languageId":"typescript","providerId":"ts-harness","files":[{"path":"src/app.ts","languageId":"javascript","providerId":"js-harness"}]}"#;

    let scope =
        provider_workspace_scope_from_stdout(stdout, &provider).expect("parse workspace scope");

    let ProviderWorkspaceScope::Supported(packet) = scope else {
        panic!("workspace-scope packet should be supported");
    };
    assert_eq!(packet.language_id.as_str(), "typescript");
    assert_eq!(packet.provider_id.as_str(), "ts-harness");
    assert_eq!(packet.files[0].language_id.as_str(), "javascript");
    assert_eq!(packet.files[0].provider_id.as_str(), "js-harness");
}

#[test]
fn workspace_scope_missing_anchor_is_supported_empty_scope() {
    let provider = provider();
    let stdout = br#"{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","status":"missing-anchor"}"#;

    let scope =
        provider_workspace_scope_from_stdout(stdout, &provider).expect("parse workspace scope");

    let ProviderWorkspaceScope::Supported(packet) = scope else {
        panic!("missing-anchor should be a supported empty scope");
    };
    assert!(packet.files.is_empty());
}

#[test]
fn workspace_scope_ignores_wrong_schema_and_empty_packets() {
    let provider = provider();

    assert_eq!(
        provider_workspace_scope_from_stdout(br#"{"schemaId":"other","files":[]}"#, &provider)
            .expect("parse workspace scope"),
        ProviderWorkspaceScope::Unsupported
    );
    assert_eq!(
        provider_workspace_scope_from_stdout(br#"{"files":[]}"#, &provider)
            .expect("parse workspace scope"),
        ProviderWorkspaceScope::Unsupported
    );
}

#[test]
fn workspace_scope_schema_constant_matches_public_contract() {
    assert_eq!(
        PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID,
        "agent.semantic-protocols.semantic-workspace-scope"
    );
}

#[test]
fn workspace_scope_packet_projection_filters_to_existing_unignored_children() {
    let root = temp_root("workspace-scope-projection");
    let package_root = root.join("pkg");
    std::fs::create_dir_all(package_root.join("src")).expect("create source dir");
    std::fs::create_dir_all(package_root.join("target")).expect("create ignored dir");
    std::fs::write(package_root.join("src/lib.rs"), "pub fn fixture() {}\n")
        .expect("write source fixture");
    std::fs::write(
        package_root.join("target/generated.rs"),
        "pub fn generated() {}\n",
    )
    .expect("write ignored fixture");
    let mut provider = provider();
    provider.ignored_path_prefixes = vec!["pkg/target".to_string()];
    let packet = ProviderWorkspaceScopePacket {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        files: vec![
            ProviderWorkspaceScopeFile {
                path: "src/lib.rs".to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
            },
            ProviderWorkspaceScopeFile {
                path: "target/generated.rs".to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
            },
            ProviderWorkspaceScopeFile {
                path: "../outside.rs".to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
            },
            ProviderWorkspaceScopeFile {
                path: "src/missing.rs".to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
            },
        ],
    };

    let files = provider_workspace_scope_files_from_packet(&root, &provider, &package_root, packet);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, package_root.join("src/lib.rs"));
    assert_eq!(files[0].language_id.as_str(), "rust");
    assert_eq!(files[0].provider_id.as_str(), "rs-harness");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_source_scope_manifest_fallback_collects_config_and_source_files() {
    let root = temp_root("provider-source-scope");
    std::fs::create_dir_all(root.join("src")).expect("create source dir");
    std::fs::create_dir_all(root.join("target")).expect("create ignored dir");
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\n")
        .expect("write config fixture");
    std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write source fixture");
    std::fs::write(root.join("target/generated.rs"), "pub fn generated() {}\n")
        .expect("write ignored fixture");

    let mut provider = provider();
    provider.source_roots = vec!["src".to_string(), "target".to_string()];
    provider.config_files = vec!["Cargo.toml".to_string()];
    provider.source_extensions = vec!["rs".to_string(), ".rs".to_string()];
    provider.ignored_path_prefixes = vec!["target".to_string()];
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("registry.json"),
        providers: vec![provider],
    };

    let files =
        collect_provider_source_scope_files(&root, &snapshot, 16).expect("collect source scope");
    let paths = files
        .iter()
        .map(|file| {
            file.path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()]
    );
    assert!(files.iter().all(|file| file.language_id.as_str() == "rust"));
    assert!(
        files
            .iter()
            .all(|file| file.provider_id.as_str() == "rs-harness")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_source_scope_workspace_scope_timeout_falls_back_to_manifest() {
    let root = temp_root("provider-source-scope-timeout");
    std::fs::create_dir_all(root.join("src")).expect("create source dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write source fixture");

    let mut provider = provider();
    provider.binary = "missing-timeout-provider".to_string();
    provider.runtime_command_argv = Some(vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 2".to_string(),
    ]);
    provider.source_roots = vec!["src".to_string()];
    provider.source_extensions = vec!["rs".to_string()];
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("registry.json"),
        providers: vec![provider],
    };

    let started = Instant::now();
    let files =
        collect_provider_source_scope_files(&root, &snapshot, 16).expect("collect source scope");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "workspace-scope timeout should fall back before provider sleep finishes"
    );
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].path.strip_prefix(&root).unwrap(),
        std::path::Path::new("src/lib.rs")
    );
    let _ = std::fs::remove_dir_all(root);
}

fn provider() -> ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust provider manifest");
    ResolvedProvider {
        manifest_id: "rust-test-manifest".to_string(),
        manifest_digest: "sha256:rust-test-manifest".to_string(),
        namespace: "rust".to_string(),
        language_id: "rust".into(),
        provider_id: "rs-harness".into(),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        execution_command_digest: "test-execution-command-digest".to_string(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: Vec::new(),
        source_roots: Vec::new(),
        config_files: Vec::new(),
        source_extensions: Vec::new(),
        ignored_path_prefixes: Vec::new(),
        search_capabilities: manifest.search_capabilities,
        query_pack_descriptor: manifest.query_pack_descriptor,
        semantic_facts_descriptor: manifest.semantic_facts_descriptor,
    }
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}
