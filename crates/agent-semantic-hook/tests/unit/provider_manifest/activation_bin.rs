use agent_semantic_hook::{build_default_activation, builtin_provider_manifests};
use std::fs;

use super::{git_init, make_executable, temp_root};

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
fn default_activation_uses_parent_workspace_bin_for_nested_gerbil_package() {
    let root = temp_root("nested-gerbil-parent-bin-provider");
    let child = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    fs::create_dir_all(root.join(".bin")).expect("create workspace bin");
    fs::create_dir_all(child.join("src")).expect("create child src");
    write_agent_config(&root, "[providers]\n");
    fs::write(child.join("gerbil.pkg"), "(package: sample/gerbil)\n").expect("write gerbil.pkg");
    let gerbil_manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("builtin Gerbil provider manifest");
    let provider_bin = root.join(".bin").join(&gerbil_manifest.binary);
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);

    let activation = build_default_activation(&child).expect("build activation");
    let gerbil = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "gerbil-scheme")
        .expect("gerbil provider activated from parent workspace .bin");

    assert_eq!(gerbil.binary, gerbil_manifest.binary);
    let expected_bin_suffix = format!("/.bin/{}", gerbil.binary);
    assert!(
        gerbil
            .provider_command_prefix
            .first()
            .is_some_and(|command| command.ends_with(&expected_bin_suffix)),
        "nested Gerbil package should reuse the parent workspace provider bin: {:?}",
        gerbil.provider_command_prefix
    );
    assert!(gerbil.coverage.package_roots.contains(&".".to_string()));

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
    write_agent_config(
        &root,
        "[languages.rust]\nbin = \"tools/custom-rs-harness\"\n",
    );

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
    write_agent_config(&root, "[providers.org]\nenabled = false\n");

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

#[test]
fn top_level_asp_toml_no_longer_configures_provider_activation() {
    let root = temp_root("legacy-top-level-ignored");
    fs::create_dir_all(root.join(".bin")).expect("create project bin");
    let asp_bin = root.join(".bin/asp");
    fs::write(&asp_bin, "#!/bin/sh\nexit 0\n").expect("write asp bin");
    make_executable(&asp_bin);
    fs::write(root.join("asp.toml"), "[providers.org]\nenabled = false\n")
        .expect("write legacy asp.toml");

    let activation = build_default_activation(&root).expect("build activation");

    assert!(
        activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "org"),
        "build_default_activation must ignore legacy top-level asp.toml; sync/install owns migration"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

fn write_agent_config(root: &std::path::Path, contents: &str) {
    let config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    fs::write(&config_path, contents).expect("write .agents/asp.toml");
}
