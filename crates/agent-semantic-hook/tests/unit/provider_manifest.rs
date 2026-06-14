use agent_semantic_hook::{build_default_activation, builtin_provider_manifests};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
    assert_eq!(julia.binary, "asp-julia-harness");
    assert!(julia.source.default_extensions.contains(&".jl".to_string()));
    assert!(
        julia
            .source
            .default_config_files
            .contains(&"Project.toml".to_string())
    );
    assert_eq!(
        julia.routes.guide.as_ref().expect("guide route").argv,
        ["asp-julia-harness", "guide", "{projectRoot}"]
    );
    assert_eq!(
        julia.routes.query.as_ref().expect("query route").argv,
        [
            "asp-julia-harness",
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
            "asp-julia-harness",
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
fn builtin_manifests_include_document_language_providers() {
    let manifests = builtin_provider_manifests();
    let org = manifests
        .iter()
        .find(|manifest| manifest.language_id == "org")
        .expect("org manifest");
    let md = manifests
        .iter()
        .find(|manifest| manifest.language_id == "md")
        .expect("md manifest");

    assert_eq!(org.provider_id, "orgize");
    assert_eq!(org.binary, "asp");
    assert_eq!(org.execution.as_str(), "embedded");
    assert!(org.source.default_extensions.contains(&".org".to_string()));
    assert_eq!(
        org.routes.query.as_ref().expect("org query route").argv,
        [
            "asp",
            "org",
            "query",
            "{termArgs}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        org.routes.owner.argv,
        [
            "asp",
            "org",
            "query",
            "--selector",
            "{path}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        org.routes.fzf.argv,
        [
            "asp",
            "org",
            "query",
            "--term",
            "{query}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );

    assert_eq!(md.provider_id, "orgize");
    assert_eq!(md.binary, "asp");
    assert_eq!(md.execution.as_str(), "embedded");
    assert!(md.source.default_extensions.contains(&".md".to_string()));
    assert_eq!(
        md.routes.query.as_ref().expect("md query route").argv,
        [
            "asp",
            "md",
            "query",
            "{termArgs}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        md.routes.owner.argv,
        [
            "asp",
            "md",
            "query",
            "--selector",
            "{path}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        md.routes.fzf.argv,
        [
            "asp",
            "md",
            "query",
            "--term",
            "{query}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
}

#[test]
fn default_activation_records_project_bin_provider_prefix() {
    let root = temp_root("julia-project-bin-provider");
    fs::create_dir_all(root.join(".bin")).expect("create project bin");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("Project.toml"), "name = \"Example\"\n").expect("write Project.toml");
    let provider_bin = root.join(".bin/asp-julia-harness");
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);

    let activation = build_default_activation(&root).expect("build activation");
    let julia = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "julia")
        .expect("julia provider activated from project .bin");

    assert_eq!(julia.binary, "asp-julia-harness");
    assert!(
        julia
            .provider_command_prefix
            .first()
            .is_some_and(|command| command.ends_with("/.bin/asp-julia-harness")),
        "default project .bin provider should be recorded as a stable command prefix: {:?}",
        julia.provider_command_prefix
    );
    assert!(julia.coverage.package_roots.contains(&".".to_string()));

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_uses_project_runtime_bin_provider_prefix() {
    let root = temp_root("runtime-bin-provider");
    git_init(&root);
    fs::create_dir_all(root.join(".cache/agent-semantic-protocol/runtime/bin"))
        .expect("create runtime bin");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"runtime-bin-provider\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    let provider_bin = root.join(".cache/agent-semantic-protocol/runtime/bin/rs-harness");
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);

    let activation = build_default_activation(&root).expect("build activation");
    let rust = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activated from project runtime bin");

    assert_eq!(rust.binary, "rs-harness");
    assert!(
        rust.provider_command_prefix
            .first()
            .is_some_and(|command| command.ends_with("/runtime/bin/rs-harness")),
        "default project runtime bin provider should be recorded as a stable command prefix: {:?}",
        rust.provider_command_prefix
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_accepts_languages_bin_provider_override() {
    let root = temp_root("languages-bin-provider-override");
    fs::create_dir_all(root.join("tools")).expect("create tools");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"languages-bin-provider-override\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    let provider_bin = root.join("tools/custom-rs-harness");
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);
    fs::write(
        root.join("asp.toml"),
        "[languages.rust]\nbin = \"tools/custom-rs-harness\"\n",
    )
    .expect("write asp.toml");

    let activation = build_default_activation(&root).expect("build activation");
    let rust = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activated from languages bin override");

    assert_eq!(rust.binary, "rs-harness");
    let expected_provider_bin =
        fs::canonicalize(&provider_bin).unwrap_or_else(|_| provider_bin.clone());
    assert_eq!(
        rust.provider_command_prefix,
        vec![expected_provider_bin.display().to_string()]
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn asp_toml_can_disable_document_language_hook_activation() {
    let root = temp_root("document-provider-disable");
    fs::create_dir_all(root.join(".bin")).expect("create project bin");
    let asp_bin = root.join(".bin/asp");
    fs::write(&asp_bin, "#!/bin/sh\nexit 0\n").expect("write asp bin");
    make_executable(&asp_bin);
    fs::write(root.join("asp.toml"), "[providers.org]\nenabled = false\n").expect("write asp.toml");

    let activation = build_default_activation(&root).expect("build activation");

    assert!(
        !activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "org")
    );
    let md = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "md")
        .expect("md provider remains enabled");
    assert_eq!(md.provider_id, "orgize");
    assert_eq!(md.binary, "asp");
    assert_eq!(md.execution.as_str(), "embedded");
    assert!(
        md.provider_command_prefix
            .first()
            .is_some_and(|command| command.ends_with("/.bin/asp")),
        "document provider should route through the project asp facade: {:?}",
        md.provider_command_prefix
    );

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

fn git_init(root: &std::path::Path) {
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .expect("run git init");
    assert!(status.success(), "git init failed with {status}");
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}
