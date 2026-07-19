use crate::{
    cache_cli::run_cache, lookup_source_index_for_language, source_index::refresh_source_index,
};
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
use serde_json::json;
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cache_source_index_refresh_builds_db_engine_rows() {
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

    let rebuild_started = std::time::Instant::now();
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    let rebuild_elapsed = rebuild_started.elapsed();
    assert!(
        rebuild_elapsed < std::time::Duration::from_secs(5),
        "source-index cold rebuild exceeded fixture gate: elapsedMs={}",
        rebuild_elapsed.as_millis()
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("reuse refreshed source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(
        engine.db_path().exists(),
        "source-index refresh must write the active DB Engine path"
    );
    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "gerbil-poo",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
    assert_eq!(result.candidates[0].line_count, Some(3));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_without_generation_is_bounded_warm_check() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-refresh-cold-required");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &[]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let started = std::time::Instant::now();
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh without generation should be a warm check");
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "source-index refresh warm check exceeded gate: elapsedMs={}",
        elapsed.as_millis()
    );
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
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
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
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild changed source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "new-scope-symbol",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "extra/new_usage.ss");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_updates_dirty_tracked_worktree() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-dirty-tracked-worktree");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    let source_path = source_dir.join("usage.ss");
    std::fs::write(
        &source_path,
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "gerbil.pkg", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild clean source index");

    std::fs::write(
        &source_path,
        "(def (poo-read input)\n  ;; gerbil-poo://dirty\n  input)\n",
    )
    .expect("dirty tracked source");
    let refreshed = refresh_source_index(&root)
        .expect("refresh dirty source index")
        .expect("refresh must publish changed tracked source without requiring rebuild");
    assert!(
        !refreshed.reused_generation,
        "the first dirty refresh must publish changed source content"
    );
    let reused = refresh_source_index(&root)
        .expect("reuse unchanged dirty source index")
        .expect("unchanged dirty source must retain its source-index generation");
    assert!(
        reused.reused_generation,
        "the second dirty refresh must hash only dirty paths and reuse the generation"
    );
    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "dirty",
        8,
    )
    .expect("lookup refreshed dirty source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
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
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "workspace_scope_symbol",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "crates/app/src/lib.rs");
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
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable/lifecycle-v1"))
        .expect("create lifecycle v1 source dir");
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable/docs"))
        .expect("create lifecycle docs source dir");
    std::fs::write(
        root.join(
            "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs",
        ),
        "pub fn codex_resident_agent_lifecycle_v1() {}\n",
    )
    .expect("write lifecycle v1 source");
    std::fs::write(
        root.join(
            "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs",
        ),
        "pub fn codex_resident_agent_lifecycle_v2() {}\n",
    )
    .expect("write lifecycle v2 source");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &[]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");

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

    let versioned_alias = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "10.15.02-codex-resident-agent-lifecycle-v2.org",
        8,
    )
    .expect("lookup lifecycle v2 alias source index");

    assert_eq!(versioned_alias.state.as_str(), "hit");
    assert!(
        versioned_alias
            .candidates
            .iter()
            .any(|candidate| candidate.path
                == "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs"),
        "{:?}",
        versioned_alias.candidates
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
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = lookup_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "provider-scope-symbol",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/included.ss");
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

fn run_git(project_root: &Path, args: impl IntoIterator<Item = &'static str>) {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .expect("run git for source-index fixture");
    assert!(
        output.status.success(),
        "git source-index fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
