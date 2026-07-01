use crate::{cache_cli::run_cache, lookup_source_index_for_language};
use agent_semantic_client_core::{
    ASP_PROVIDER_ACTIVATION_PATH_ENV, ClientCacheManifest, LanguageId,
};
#[cfg(feature = "turso-backend")]
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::{ClientDb, ClientDbSourceIndexLookup, ClientDbSourceIndexQueryKey};
use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
#[cfg(feature = "turso-backend")]
use agent_semantic_runtime::runtime_block_on_current_thread;
use serde_json::json;
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cache_source_index_refresh_builds_rust_sql_rows() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-refresh");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        source_dir.join("usage.ss"),
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    std::fs::create_dir_all(root.join(".data/codex")).expect("create external data dir");
    std::fs::write(
        root.join(".data/codex/usage.rs"),
        "fn external_gerbil_poo_usage() {}\n",
    )
    .expect("write ignored data source");
    std::fs::create_dir_all(root.join(".codex/plugins/cache")).expect("create plugin cache dir");
    std::fs::write(
        root.join(".codex/plugins/cache/usage.rs"),
        "fn plugin_cache_gerbil_poo_usage() {}\n",
    )
    .expect("write ignored plugin cache source");

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh source index");
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("reuse refreshed source index");

    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");
    let owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("gerbil-scheme")),
            query: ClientDbSourceIndexQueryKey::from("gerbil-poo"),
            limit: 8,
        })
        .expect("lookup source owners");

    assert_eq!(summary.source_index_generation_count, 1);
    assert_eq!(summary.source_index_owner_count, 2);
    assert_eq!(summary.source_index_selector_count, 2);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "src/usage.ss");
    assert_eq!(owners[0].line_count, Some(3));
    #[cfg(feature = "turso-backend")]
    {
        let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
        assert!(
            engine.db_path().exists(),
            "source-index refresh must project stable rows into active Turso read model"
        );
        let hits =
            runtime_block_on_current_thread(engine.search_source_index_documents("gerbil-poo", 8))
                .expect("run Turso source-index search")
                .expect("search Turso source-index documents through DB Engine facade");
        assert!(
            hits.iter().any(|hit| {
                hit.source == "stable"
                    && hit.selector.as_deref() == Some("gerbil-scheme://src/usage.ss#file")
            }),
            "hits={hits:?}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_invalidates_when_empty_source_root_gains_file() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-empty-root-invalidates");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    let extra_dir = root.join("extra");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::create_dir_all(&extra_dir).expect("create empty extra source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        source_dir.join("usage.ss"),
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path = write_gerbil_activation_with_command_prefix(
        &root,
        vec!["true".to_string()],
        &["src", "extra"],
    );
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh source index");
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("reuse source index");
    std::fs::write(
        extra_dir.join("new_usage.ss"),
        "(def (new-scope-symbol input)\n  input)\n",
    )
    .expect("write new extra source");
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh changed source index");

    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");
    let owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("gerbil-scheme")),
            query: ClientDbSourceIndexQueryKey::from("new-scope-symbol"),
            limit: 8,
        })
        .expect("lookup source owners");

    assert_eq!(summary.source_index_generation_count, 2);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "extra/new_usage.ss");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_respects_provider_ignored_path_prefixes() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-workspace-exclude");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/app\", \"vendor/tool\"]\nexclude = [\"vendor/tool\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    std::fs::create_dir_all(root.join("crates/app/src")).expect("create app source dir");
    std::fs::write(
        root.join("crates/app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write app manifest");
    std::fs::write(
        root.join("crates/app/src/lib.rs"),
        "pub fn workspace_scope_symbol() {}\n",
    )
    .expect("write app source");
    std::fs::create_dir_all(root.join("vendor/tool/src")).expect("create excluded source dir");
    std::fs::write(
        root.join("vendor/tool/Cargo.toml"),
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write excluded manifest");
    std::fs::write(
        root.join("vendor/tool/src/lib.rs"),
        "pub fn workspace_scope_symbol() {}\n",
    )
    .expect("write excluded source");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &["vendor"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh source index");

    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("rust")),
            query: ClientDbSourceIndexQueryKey::from("workspace_scope_symbol"),
            limit: 8,
        })
        .expect("lookup source owners");

    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "crates/app/src/lib.rs");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_lookup_ranks_query_dense_owner_before_low_coverage_path() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-query-axis-rank");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/app\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable"))
        .expect("create package source dir");
    std::fs::write(
        root.join("crates/app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write app manifest");
    std::fs::write(
        root.join("crates/app/src/semantic_sandtable/surface.rs"),
        "pub fn ablation_surface() {}\n",
    )
    .expect("write low coverage source");
    std::fs::write(
        root.join("crates/app/src/semantic_sandtable/report_chain.rs"),
        "pub fn topology_membership_report_chain_request_policy() {}\n",
    )
    .expect("write report chain source");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &[]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh source index");

    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "ablation sandtable topology membership report chain request policy",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(
        result.candidates[0].path,
        "crates/app/src/semantic_sandtable/report_chain.rs"
    );
    assert!(
        result
            .candidates
            .iter()
            .any(|candidate| candidate.path == "crates/app/src/semantic_sandtable/surface.rs"),
        "{:?}",
        result.candidates
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_prefers_provider_workspace_scope_packet() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-provider-workspace-scope");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package source-index-provider-scope)\n",
    )
    .expect("write gerbil package anchor");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("extra")).expect("create extra");
    std::fs::write(
        root.join("src/included.ss"),
        "(def (provider-scope-symbol) 'included)\n",
    )
    .expect("write included source");
    std::fs::write(
        root.join("extra/excluded.ss"),
        "(def (provider-scope-symbol) 'excluded)\n",
    )
    .expect("write excluded source");
    let provider_bin = home_local_provider_path(&root, "gslph");
    std::fs::create_dir_all(provider_bin.parent().expect("provider parent"))
        .expect("create home local bin");
    std::fs::write(
        &provider_bin,
        r#"#!/bin/sh
if [ "$1" = "search" ] && [ "$2" = "workspace-scope" ]; then
  printf '%s\n' '{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","schemaVersion":"1","status":"ready","languageId":"gerbil-scheme","providerId":"gerbil-scheme-harness","files":[{"path":"src/included.ss"}]}'
  exit 0
fi
exit 2
"#,
    )
    .expect("write fake provider");
    make_executable(&provider_bin);
    let activation_path =
        write_gerbil_activation_with_provider_scope(&root, &provider_bin, &["src", "extra"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh source index");

    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("gerbil-scheme")),
            query: ClientDbSourceIndexQueryKey::from("provider-scope-symbol"),
            limit: 8,
        })
        .expect("lookup source owners");

    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "src/included.ss");
    let _ = std::fs::remove_dir_all(root);
}

fn write_rust_activation_with_ignored_prefixes(
    root: &Path,
    ignored: &[&str],
) -> std::path::PathBuf {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": activation_project_root.display().to_string(),
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "execution": manifest.execution,
            "providerCommandPrefix": ["true"],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": ["crates/app/src", "vendor/tool/src"],
                "configFiles": ["Cargo.toml", "crates/app/Cargo.toml", "vendor/tool/Cargo.toml"],
                "sourceExtensions": ["rs"],
                "ignoredPathPrefixes": ignored
            }
        }]
    });
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("activation json"),
    )
    .expect("write activation");
    activation_path
}

fn write_gerbil_activation_with_provider_scope(
    root: &Path,
    provider_bin: &Path,
    source_roots: &[&str],
) -> std::path::PathBuf {
    write_gerbil_activation_with_command_prefix(
        root,
        vec![provider_bin.display().to_string()],
        source_roots,
    )
}

fn write_gerbil_activation_with_command_prefix(
    root: &Path,
    provider_command_prefix: Vec<String>,
    source_roots: &[&str],
) -> std::path::PathBuf {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("gerbil manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": activation_project_root.display().to_string(),
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "execution": manifest.execution,
            "providerCommandPrefix": provider_command_prefix,
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": source_roots,
                "configFiles": ["gerbil.pkg"],
                "sourceExtensions": ["ss"],
                "ignoredPathPrefixes": []
            }
        }]
    });
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("activation json"),
    )
    .expect("write activation");
    activation_path
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set executable");
    }
}

fn isolate_home(root: &Path) -> EnvVarGuard {
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create isolated home");
    EnvVarGuard::set("HOME", home.as_os_str())
}

fn home_local_provider_path(root: &Path, binary: &str) -> std::path::PathBuf {
    root.join("home").join(".local/bin").join(binary)
}

struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(name: &'static str, value: &OsStr) -> Self {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(self.name, value);
            } else {
                std::env::remove_var(self.name);
            }
        }
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-client-source-index-{label}-{nanos}"));
    std::fs::create_dir_all(root.join(".git")).expect("create temp project root");
    root
}
