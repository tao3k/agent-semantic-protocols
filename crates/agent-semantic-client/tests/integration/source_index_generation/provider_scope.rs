use agent_semantic_client::source_index::{
    ProviderSourceEnvelopeLookupRequestV1, SourceIndexCollectionScope,
    TargetProviderSourceEnvelopePublicationRequestV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry as current_provider_snapshot,
    ensure_provider_source_index_snapshot_at_artifact_root_with_registry as ensure_provider_snapshot,
    publish_target_provider_source_envelope_v1,
};
use agent_semantic_client_core::{ProviderExecution, ProviderRegistrySnapshot, ResolvedProvider};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-source-index-provider-scope-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create provider scope test repository");
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .arg(&root)
        .status()
        .expect("launch git init for provider scope test repository");
    assert!(
        status.success(),
        "initialize provider scope test repository"
    );
    root
}

struct ProviderTestEnvironment {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_home: Option<std::ffi::OsString>,
    artifact_root: std::path::PathBuf,
}

impl ProviderTestEnvironment {
    fn enter(root: &std::path::Path) -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let lock = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let home = root.join("home");
        std::fs::create_dir_all(&home).expect("create isolated provider test home");
        let previous_home = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", &home);
        }
        Self {
            _lock: lock,
            previous_home,
            artifact_root: root.with_extension("artifacts"),
        }
    }
}

impl Drop for ProviderTestEnvironment {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.artifact_root);
        unsafe {
            if let Some(home) = &self.previous_home {
                std::env::set_var("HOME", home);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}

fn provider(
    language_id: &str,
    provider_id: &str,
    source_root: &str,
    source_extension: &str,
) -> ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .expect("builtin provider manifest");
    let (project_entry, parser_id) = match language_id {
        "rust" => ("Cargo.toml", "rust.cargo-toml"),
        "gerbil-scheme" => ("gerbil.pkg", "gerbil.package-spec"),
        "python" => ("pyproject.toml", "python.pyproject-toml"),
        _ => ("Project.toml", "julia.pkg-project-toml"),
    };
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/project_resolution_provider.py");
    let fixture_config = serde_json::json!({
        "languageId": language_id,
        "providerId": provider_id,
        "sourceRoot": source_root,
        "extension": source_extension,
        "projectEntry": project_entry,
        "parserId": parser_id,
    });
    static PROVIDER_FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let fixture_id = PROVIDER_FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let binary = format!("{provider_id}-{fixture_id}.py");
    let wrapper =
        std::path::PathBuf::from(std::env::var_os("HOME").expect("isolated provider test HOME"))
            .join(".agent-semantic-protocols/runtime/bin")
            .join(&binary);
    std::fs::create_dir_all(wrapper.parent().expect("provider fixture wrapper parent"))
        .expect("create provider fixture wrapper parent");
    std::fs::write(
        &wrapper,
        format!(
            "#!/usr/bin/env python3\nimport os\nimport sys\nos.execvp(\"python3\", [\"python3\", {fixture:?}, {config:?}, *sys.argv[1:]])\n",
            fixture = fixture.display().to_string(),
            config = fixture_config.to_string(),
        ),
    )
    .expect("write project-resolution provider wrapper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&wrapper)
            .expect("read project-resolution provider wrapper metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&wrapper, permissions)
            .expect("make project-resolution provider wrapper executable");
    }
    ResolvedProvider {
        manifest_id: format!("{provider_id}-test-manifest"),
        manifest_digest: format!("sha256:{provider_id}-test-manifest"),
        namespace: language_id.to_string(),
        language_id: language_id.into(),
        provider_id: provider_id.into(),
        binary,
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        execution_command_digest: format!("{provider_id}-test-execution-command-digest"),
        runtime_command_argv: None,
        runtime_profile_status: Some(agent_semantic_client_core::RuntimeProfileStatus::Available),
        package_roots: Vec::new(),
        config_files: Vec::new(),
        source_extensions: vec![source_extension.to_string()],
        scope_authority: agent_semantic_client_core::ProviderScopeAuthority::ProjectResolution,
        search_capabilities: manifest.search_capabilities().clone(),
        language_projection: manifest.language_projection().cloned(),
        query_pack_descriptor: manifest.query_pack_descriptor().clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
    }
}

fn current_provider_source_index_snapshot_at_artifact_root_with_registry(
    project_root: &std::path::Path,
    artifact_root: &std::path::Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<agent_semantic_client::source_index::CurrentSourceIndexSnapshot, String> {
    current_provider_snapshot(ProviderSourceEnvelopeLookupRequestV1 {
        project_root,
        artifact_root,
        language_id,
        provider_id,
        provider_registry,
    })
}

fn ensure_provider_source_index_snapshot_at_artifact_root_with_registry(
    project_root: &std::path::Path,
    artifact_root: &std::path::Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    provider_registry: &ProviderRegistrySnapshot,
) -> Result<agent_semantic_client::source_index::CurrentSourceIndexSnapshot, String> {
    ensure_provider_snapshot(ProviderSourceEnvelopeLookupRequestV1 {
        project_root,
        artifact_root,
        language_id,
        provider_id,
        provider_registry,
    })
}

#[test]
fn target_provider_publication_does_not_require_complete_generation() {
    let root = test_root("target-publication");
    let _environment = ProviderTestEnvironment::enter(&root);
    std::fs::create_dir_all(root.join("rust-src")).expect("create rust source root");
    std::fs::write(root.join("rust-src/lib.rs"), "pub fn rust_owner() {}\n")
        .expect("write rust owner");
    let mut rust = provider("rust", "rs-harness", "rust-src", "rs");
    rust.source_extensions.clear();
    rust.config_files = vec!["rust-src/lib.rs".to_string()];
    let missing = provider(
        "julia",
        "julia-lang-project-harness",
        "missing-julia-src",
        "jl",
    );
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust, missing],
    };
    let first_live =
        agent_semantic_client::source_index::current_live_provider_source_index_snapshot_with_registry(
            &root,
            &"rust".into(),
            &"rs-harness".into(),
            &provider_registry,
        )
        .expect("collect first deterministic live provider snapshot");
    let second_live =
        agent_semantic_client::source_index::current_live_provider_source_index_snapshot_with_registry(
            &root,
            &"rust".into(),
            &"rs-harness".into(),
            &provider_registry,
        )
        .expect("collect second deterministic live provider snapshot");
    assert_eq!(first_live.source_snapshot, second_live.source_snapshot);
    let artifact_root = root.with_extension("artifacts");

    let envelope = publish_target_provider_source_envelope_v1(
        TargetProviderSourceEnvelopePublicationRequestV1 {
            collection_scope: SourceIndexCollectionScope::TargetProvider {
                language_id: "rust".into(),
                provider_id: "rs-harness".into(),
            },
            provider_registry: &provider_registry,
            artifact_root: &artifact_root,
            project_root: &root,
        },
    )
    .expect("publish target provider without unrelated provider coverage");

    assert!(envelope.is_file());
    let after_publication =
        agent_semantic_client::source_index::current_live_provider_source_index_snapshot_with_registry(
            &root,
            &"rust".into(),
            &"rs-harness".into(),
            &provider_registry,
        )
        .expect("collect deterministic live provider snapshot after publication");
    assert_eq!(
        second_live.source_snapshot,
        after_publication.source_snapshot
    );
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&envelope).expect("read provider source envelope"))
            .expect("decode provider source envelope");
    assert_eq!(value["providerId"], "rs-harness");
    assert_eq!(
        value["addressProviderDigest"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(value["ownerCoverage"], "complete");
    assert_eq!(value["owners"].as_array().map(Vec::len), Some(1));
    assert!(!artifact_root.join("source-index-generations").exists());
    let stale = envelope
        .parent()
        .expect("canonical envelope directory")
        .join("rs-harness--stale-registry-digest.json");
    std::fs::write(&stale, b"{\"tampered\":true}").expect("write stale provider envelope");
    let loaded = current_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"rust".into(),
        &"rs-harness".into(),
        &provider_registry,
    )
    .expect("load the freshly published target provider");
    assert_eq!(loaded.source_blobs.iter().count(), 1);
    assert_eq!(
        loaded.source_snapshot.root_digest,
        value["sourceSnapshot"]["rootDigest"]
            .as_str()
            .expect("published root digest")
    );
    let mut wrong_address = value.clone();
    wrong_address["addressProviderDigest"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    std::fs::write(
        &envelope,
        serde_json::to_vec_pretty(&wrong_address).expect("encode wrong-address envelope"),
    )
    .expect("write wrong-address envelope");
    let wrong_address_error =
        match current_provider_source_index_snapshot_at_artifact_root_with_registry(
            &root,
            &artifact_root,
            &"rust".into(),
            &"rs-harness".into(),
            &provider_registry,
        ) {
            Ok(_) => panic!("address-provider digest mismatch must fail closed"),
            Err(error) => error,
        };
    assert!(
        wrong_address_error.contains("reason=address-provider-digest"),
        "{wrong_address_error}"
    );
    std::fs::write(
        &envelope,
        serde_json::to_vec_pretty(&value).expect("restore provider envelope"),
    )
    .expect("restore provider envelope");
    let mut empty_envelope = value.clone();
    empty_envelope["owners"] = serde_json::json!([]);
    empty_envelope["sourceSnapshot"]["leafCount"] = serde_json::json!(0);
    std::fs::write(
        &envelope,
        serde_json::to_vec_pretty(&empty_envelope).expect("encode empty envelope"),
    )
    .expect("write empty provider envelope");
    let empty_error = current_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"rust".into(),
        &"rs-harness".into(),
        &provider_registry,
    )
    .err()
    .expect("empty requested provider envelope must fail closed");
    assert!(empty_error.contains("reason=owners-empty"));
    std::fs::remove_file(&envelope).expect("remove requested provider envelope");
    let missing_error = match current_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"rust".into(),
        &"rs-harness".into(),
        &provider_registry,
    ) {
        Ok(_) => panic!("exact-query consumer must not materialize a missing envelope"),
        Err(error) => error,
    };
    assert!(missing_error.contains("source envelope is not published"));
    assert!(!envelope.is_file());
    let rematerialized = ensure_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"rust".into(),
        &"rs-harness".into(),
        &provider_registry,
    )
    .expect("reasoning search must explicitly materialize the requested provider");
    assert_eq!(rematerialized.source_blobs.iter().count(), 1);
    assert!(envelope.is_file());
    assert!(!artifact_root.join("source-index-generations").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn target_provider_live_snapshot_does_not_require_published_envelope() {
    let root = test_root("provider-live-without-envelope");
    let _environment = ProviderTestEnvironment::enter(&root);
    std::fs::create_dir_all(&root).expect("create Gerbil workspace");
    std::fs::write(root.join("gerbil.pkg"), "package: gslph\n").expect("write Gerbil anchor");
    std::fs::write(root.join("build.ss"), "(displayln \"build\")\n")
        .expect("write Gerbil build source");
    let gerbil = provider("gerbil-scheme", "gerbil-scheme-harness", ".", ".ss");
    let unrelated = provider("rust", "rs-harness", "missing-rust-src", ".rs");
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![gerbil, unrelated],
    };

    let snapshot =
        agent_semantic_client::source_index::current_live_provider_source_index_snapshot_with_registry(
            &root,
            &"gerbil-scheme".into(),
            &"gerbil-scheme-harness".into(),
            &provider_registry,
        )
        .expect("capture target provider directly from the live worktree");
    assert_eq!(snapshot.source_blobs.iter().count(), 1);
    assert_eq!(
        snapshot.source_blobs.iter().next().map(|source| source.0),
        Some("build.ss")
    );
    assert!(!root.join("artifacts").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn target_provider_gerbil_envelope_shape_publishes_source_owner() {
    let root = test_root("gerbil-envelope-shape");
    let _environment = ProviderTestEnvironment::enter(&root);
    std::fs::create_dir_all(&root).expect("create Gerbil workspace");
    std::fs::write(root.join("gerbil.pkg"), "package: gslph\n").expect("write Gerbil anchor");
    std::fs::write(root.join("build.ss"), "(displayln \"build\")\n")
        .expect("write Gerbil build source");
    let gerbil = provider("gerbil-scheme", "gerbil-scheme-harness", ".", ".ss");
    let unrelated = provider("rust", "rs-harness", "missing-rust-src", ".rs");
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![gerbil, unrelated],
    };
    let artifact_root = root.with_extension("artifacts");

    let envelope = publish_target_provider_source_envelope_v1(
        TargetProviderSourceEnvelopePublicationRequestV1 {
            collection_scope: SourceIndexCollectionScope::TargetProvider {
                language_id: "gerbil-scheme".into(),
                provider_id: "gerbil-scheme-harness".into(),
            },
            provider_registry: &provider_registry,
            artifact_root: &artifact_root,
            project_root: &root,
        },
    )
    .expect("publish real Gerbil provider envelope shape");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&envelope).expect("read Gerbil envelope"))
            .expect("decode Gerbil envelope");
    assert_eq!(value["sourceSnapshot"]["leafCount"], 1);
    assert_eq!(value["owners"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["owners"][0]["path"], "build.ss");

    let loaded = current_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"gerbil-scheme".into(),
        &"gerbil-scheme-harness".into(),
        &provider_registry,
    )
    .expect("load Gerbil envelope");
    assert_eq!(loaded.source_blobs.iter().count(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn target_provider_id_publication_materializes_only_the_registered_provider() {
    let root = test_root("target-provider-id-publication");
    let _environment = ProviderTestEnvironment::enter(&root);
    std::fs::create_dir_all(&root).expect("create provider-id workspace");
    std::fs::write(root.join("gerbil.pkg"), "package: gslph\n").expect("write Gerbil anchor");
    std::fs::write(root.join("build.ss"), "(displayln \"build\")\n").expect("write Gerbil source");
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![
            provider("gerbil-scheme", "gerbil-scheme-harness", ".", ".ss"),
            provider("rust", "rs-harness", "missing-rust-src", ".rs"),
        ],
    };
    let artifact_root = root.with_extension("artifacts");

    let envelope = publish_target_provider_source_envelope_v1(
        TargetProviderSourceEnvelopePublicationRequestV1 {
            collection_scope: SourceIndexCollectionScope::TargetProviderId {
                provider_id: "gerbil-scheme-harness".into(),
            },
            provider_registry: &provider_registry,
            artifact_root: &artifact_root,
            project_root: &root,
        },
    )
    .expect("registered provider id must publish independently");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&envelope).expect("read provider-id envelope"))
            .expect("decode provider-id envelope");
    assert_eq!(value["providerId"], "gerbil-scheme-harness");
    assert_eq!(value["sourceSnapshot"]["leafCount"], 1);
    assert_eq!(value["owners"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["owners"][0]["path"], "build.ss");
    assert!(!artifact_root.join("source-index-generations").exists());

    let loaded = current_provider_source_index_snapshot_at_artifact_root_with_registry(
        &root,
        &artifact_root,
        &"gerbil-scheme".into(),
        &"gerbil-scheme-harness".into(),
        &provider_registry,
    )
    .expect("consume provider-id envelope through the current provider path");
    assert_eq!(loaded.source_blobs.iter().count(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn same_provider_workspaces_publish_order_independent_envelopes() {
    let root = test_root("same-provider-workspaces");
    let _environment = ProviderTestEnvironment::enter(&root);
    std::fs::create_dir_all(root.join(".git")).expect("create repository marker");
    let hook_root = root.join("crates/agent-semantic-hook");
    let protocol_root = root.join("crates/agent-semantic-protocol");
    std::fs::create_dir_all(hook_root.join("src")).expect("create hook source root");
    std::fs::create_dir_all(protocol_root.join("src")).expect("create protocol source root");
    std::fs::write(
        hook_root.join("src/lib.rs"),
        "pub fn hook_workspace_owner() {}\n",
    )
    .expect("write hook owner");
    std::fs::write(
        protocol_root.join("src/lib.rs"),
        "pub fn protocol_workspace_owner() {}\n",
    )
    .expect("write protocol owner");
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![provider("rust", "rs-harness", "src", "rs")],
    };
    let artifact_root = root.with_extension("artifacts");
    let publish = |provider_workspace_root: &std::path::Path| {
        publish_target_provider_source_envelope_v1(
            TargetProviderSourceEnvelopePublicationRequestV1 {
                collection_scope: SourceIndexCollectionScope::TargetProvider {
                    language_id: "rust".into(),
                    provider_id: "rs-harness".into(),
                },
                provider_registry: &provider_registry,
                artifact_root: &artifact_root,
                project_root: provider_workspace_root,
            },
        )
        .expect("publish provider workspace envelope")
    };
    let load = |provider_workspace_root: &std::path::Path| {
        current_provider_source_index_snapshot_at_artifact_root_with_registry(
            provider_workspace_root,
            &artifact_root,
            &"rust".into(),
            &"rs-harness".into(),
            &provider_registry,
        )
        .expect("load provider workspace envelope")
    };

    let hook_envelope = publish(&hook_root);
    let protocol_envelope = publish(&protocol_root);
    assert_ne!(hook_envelope, protocol_envelope);
    assert!(hook_envelope.is_file());
    assert!(protocol_envelope.is_file());
    let hook_envelope_value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&hook_envelope).expect("read hook envelope"))
            .expect("decode hook envelope");
    let protocol_envelope_value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&protocol_envelope).expect("read protocol envelope"))
            .expect("decode protocol envelope");
    assert_ne!(
        hook_envelope_value["sourceSnapshot"]["providerDigest"],
        protocol_envelope_value["sourceSnapshot"]["providerDigest"]
    );
    assert_eq!(
        hook_envelope_value["providerWorkspaceRoot"],
        "crates/agent-semantic-hook"
    );
    assert_eq!(
        protocol_envelope_value["providerWorkspaceRoot"],
        "crates/agent-semantic-protocol"
    );
    assert_ne!(
        hook_envelope_value["providerWorkspaceIdentityDigest"],
        protocol_envelope_value["providerWorkspaceIdentityDigest"]
    );
    let hook_published_at = std::fs::metadata(&hook_envelope)
        .and_then(|metadata| metadata.modified())
        .expect("hook envelope publication time");
    let protocol_published_at = std::fs::metadata(&protocol_envelope)
        .and_then(|metadata| metadata.modified())
        .expect("protocol envelope publication time");
    let hook_snapshot = load(&hook_root);
    let protocol_snapshot = load(&protocol_root);
    let _hook_warm_snapshot = load(&hook_root);
    let _protocol_warm_snapshot = load(&protocol_root);
    assert_eq!(
        std::fs::metadata(&hook_envelope)
            .and_then(|metadata| metadata.modified())
            .expect("hook envelope warm-query time"),
        hook_published_at
    );
    assert_eq!(
        std::fs::metadata(&protocol_envelope)
            .and_then(|metadata| metadata.modified())
            .expect("protocol envelope warm-query time"),
        protocol_published_at
    );
    assert_eq!(
        hook_snapshot
            .source_blobs
            .get(&agent_semantic_client_db::ClientDbSourceIndexPath::new(
                "src/lib.rs".to_owned()
            )),
        Some(b"pub fn hook_workspace_owner() {}\n".as_slice())
    );
    assert_eq!(
        protocol_snapshot.source_blobs.get(
            &agent_semantic_client_db::ClientDbSourceIndexPath::new("src/lib.rs".to_owned())
        ),
        Some(b"pub fn protocol_workspace_owner() {}\n".as_slice())
    );

    publish(&protocol_root);
    publish(&hook_root);
    assert_eq!(
        load(&hook_root)
            .source_blobs
            .get(&agent_semantic_client_db::ClientDbSourceIndexPath::new(
                "src/lib.rs".to_owned()
            )),
        Some(b"pub fn hook_workspace_owner() {}\n".as_slice())
    );
    assert_eq!(
        load(&protocol_root).source_blobs.get(
            &agent_semantic_client_db::ClientDbSourceIndexPath::new("src/lib.rs".to_owned())
        ),
        Some(b"pub fn protocol_workspace_owner() {}\n".as_slice())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn target_provider_id_publication_fails_closed_when_provider_is_missing() {
    let root = test_root("missing-target-publication");
    let _environment = ProviderTestEnvironment::enter(&root);
    let provider_registry = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![provider("rust", "rs-harness", "rust-src", "rs")],
    };
    let artifact_root = root.with_extension("artifacts");

    let error = publish_target_provider_source_envelope_v1(
        TargetProviderSourceEnvelopePublicationRequestV1 {
            collection_scope: SourceIndexCollectionScope::TargetProviderId {
                provider_id: "missing-harness".into(),
            },
            provider_registry: &provider_registry,
            artifact_root: &artifact_root,
            project_root: &root,
        },
    )
    .expect_err("missing requested provider must fail closed");

    assert!(error.contains("requested target provider is not registered"));
    assert!(error.contains("providerId=missing-harness"));
    assert!(!artifact_root.exists());
    let _ = std::fs::remove_dir_all(root);
}
