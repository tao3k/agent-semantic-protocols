use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    RuntimeSourceSpec, collect_runtime_source_index_files, ensure_runtime_source_checkout,
    runtime_source_checkout_dir, runtime_source_index_context, runtime_source_registry_fingerprint,
};
use crate::state_core::ResolvedState;

#[test]
fn runtime_source_dir_uses_client_cache_namespace() {
    let root = temp_root("runtime-source-dir");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let checkout_dir =
        runtime_source_checkout_dir(&package_root, "runtime-source/gerbil-scheme", "v0.18.2")
            .expect("runtime source checkout dir");

    assert_eq!(
        checkout_dir,
        expected_runtime_source_dir(&package_root, "runtime-source/gerbil-scheme", "v0.18.2",)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_source_dir_rejects_path_escape_segments() {
    let root = temp_root("runtime-source-invalid-segment");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let error = runtime_source_checkout_dir(&root, "runtime-source/../gerbil-scheme", "v0.18.2")
        .expect_err("reject parent path segment");
    assert!(error.contains("invalid runtime source path segment"));

    let error = runtime_source_checkout_dir(&root, "runtime-source/gerbil-scheme", "v0.18.2/alt")
        .expect_err("reject checkout path segment");
    assert!(error.contains("invalid runtime source path segment"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_source_index_context_is_owned_by_runtime() {
    let root = temp_root("runtime-source-index-context");
    let cache_dir = root.join("client");
    let checkout_root = cache_dir.join("runtime-source/python/v1");
    fs::create_dir_all(&checkout_root).expect("create checkout root");

    let context =
        runtime_source_index_context(&checkout_root, &cache_dir, "python", "python-harness")
            .expect("runtime source index context");

    assert_eq!(context.checkout_root, canonical(&checkout_root));
    assert_eq!(
        context.registry_fingerprint,
        runtime_source_registry_fingerprint(&canonical(&checkout_root), "python", "python-harness")
    );
    assert!(context.registry_fingerprint.contains("runtimeSource\n"));
    assert!(context.registry_fingerprint.contains("language=python"));
    assert!(
        context
            .registry_fingerprint
            .contains("provider=python-harness")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_source_index_context_rejects_checkouts_outside_client_cache() {
    let root = temp_root("runtime-source-index-context-outside-cache");
    let cache_dir = root.join("client");
    let checkout_root = root.join("outside/runtime-source/python/v1");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    fs::create_dir_all(&checkout_root).expect("create checkout root");

    let error =
        runtime_source_index_context(&checkout_root, &cache_dir, "python", "python-harness")
            .expect_err("checkout outside cache must fail");

    assert!(error.contains("outside ASP client cache"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_source_index_files_are_collected_by_runtime() {
    let root = temp_root("runtime-source-index-files");
    fs::create_dir_all(root.join("src/nested")).expect("create runtime source dir");
    fs::create_dir_all(root.join(".git")).expect("create git dir");
    fs::write(root.join("src/lib.rs"), "pub fn runtime_fixture() {}\n").expect("write rust file");
    fs::write(root.join("src/nested/mod.rs"), "pub mod nested {}\n").expect("write nested rust");
    fs::write(root.join("src/readme.md"), "# ignored\n").expect("write ignored extension");
    fs::write(root.join(".git/ignored.rs"), "pub fn ignored() {}\n").expect("write vcs file");

    let files = collect_runtime_source_index_files(&root, "rust", "rs-harness", 8)
        .expect("collect runtime source index files");

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, root.join("src/lib.rs"));
    assert_eq!(files[0].language_id, "rust");
    assert_eq!(files[0].provider_id, "rs-harness");
    assert_eq!(files[1].path, root.join("src/nested/mod.rs"));

    let limited = collect_runtime_source_index_files(&root, "rust", "rs-harness", 1)
        .expect("collect limited runtime source index files");
    assert_eq!(limited.len(), 1);

    let unknown = collect_runtime_source_index_files(&root, "unknown", "unknown-harness", 8)
        .expect("collect unknown language runtime source index files");
    assert!(unknown.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_source_acquisition_clones_and_checks_out_version() {
    let root = temp_root("runtime-source-acquire");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let upstream = root.join("upstream-gerbil");
    create_tagged_repo(&upstream, "v0.18.2");

    let spec = RuntimeSourceSpec {
        language_id: "gerbil-scheme".to_string(),
        repository: upstream.display().to_string(),
        checkout: "v0.18.2".to_string(),
        state_namespace: "runtime-source/gerbil-scheme".to_string(),
        index_owner: "asp-structural-index".to_string(),
    };

    let checkout = ensure_runtime_source_checkout(&root, &spec).expect("runtime source checkout");

    assert_eq!(checkout.language_id, "gerbil-scheme");
    assert_eq!(checkout.state_namespace, "runtime-source/gerbil-scheme");
    assert_eq!(checkout.index_owner, "asp-structural-index");
    assert_eq!(
        checkout.checkout_dir,
        expected_runtime_source_dir(&root, "runtime-source/gerbil-scheme", "v0.18.2")
    );
    assert_eq!(
        fs::read_to_string(checkout.checkout_dir.join("runtime.ss")).expect("runtime source file"),
        ";; runtime source fixture\n"
    );

    let _ = fs::remove_dir_all(root);
}

fn expected_runtime_source_dir(project_root: &Path, namespace: &str, checkout: &str) -> PathBuf {
    ResolvedState::resolve(project_root)
        .expect("resolve state")
        .paths
        .client_dir
        .join(namespace)
        .join(checkout)
}

fn create_tagged_repo(repo: &Path, tag: &str) {
    fs::create_dir_all(repo).expect("create upstream repo");
    git(repo, ["init"]);
    fs::write(repo.join("runtime.ss"), ";; runtime source fixture\n").expect("write fixture");
    git(repo, ["add", "."]);
    git(
        repo,
        [
            "-c",
            "user.name=ASP Test",
            "-c",
            "user.email=asp-test@example.invalid",
            "commit",
            "-m",
            "runtime source fixture",
        ],
    );
    git(repo, ["tag", tag]);
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run git");
    assert!(status.success(), "git failed in {}", cwd.display());
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-runtime-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
