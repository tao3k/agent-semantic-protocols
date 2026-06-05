use agent_semantic_hook::{build_default_activation, builtin_provider_manifests};
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn builtin_manifests_include_julia_juliac_provider() {
    let manifests = builtin_provider_manifests();
    let julia = manifests
        .iter()
        .find(|manifest| manifest.language_id == "julia")
        .expect("julia manifest");

    assert_eq!(julia.provider_id, "julia-lang-project-harness");
    assert_eq!(julia.binary, "aslp-julia-harness");
    assert!(julia.source.default_extensions.contains(&".jl".to_string()));
    assert!(
        julia
            .source
            .default_config_files
            .contains(&"Project.toml".to_string())
    );
    assert_eq!(
        julia.routes.guide.as_ref().expect("guide route").argv,
        ["aslp-julia-harness", "guide", "{projectRoot}"]
    );
    assert_eq!(
        julia.routes.query.as_ref().expect("query route").argv,
        [
            "aslp-julia-harness",
            "search",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        julia.routes.ingest.argv,
        [
            "aslp-julia-harness",
            "search",
            "ingest",
            "owner",
            "tests",
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
    assert!(
        julia
            .source
            .default_ignored_path_prefixes
            .contains(&".devenv".to_string())
    );
}

#[test]
fn default_activation_records_project_bin_provider_prefix() {
    let root = temp_root("julia-project-bin-provider");
    fs::create_dir_all(root.join(".bin")).expect("create project bin");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("Project.toml"), "name = \"Example\"\n").expect("write Project.toml");
    let provider_bin = root.join(".bin/aslp-julia-harness");
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);

    let activation = build_default_activation(&root).expect("build activation");
    let julia = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "julia")
        .expect("julia provider activated from project .bin");

    assert_eq!(julia.binary, "aslp-julia-harness");
    assert!(
        julia
            .provider_command_prefix
            .first()
            .is_some_and(|command| command.ends_with("/.bin/aslp-julia-harness")),
        "default project .bin provider should be recorded as a stable command prefix: {:?}",
        julia.provider_command_prefix
    );
    assert!(julia.coverage.package_roots.contains(&".".to_string()));

    fs::remove_dir_all(root).expect("remove temp root");
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}
