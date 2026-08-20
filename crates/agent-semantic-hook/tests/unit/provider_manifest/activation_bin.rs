use agent_semantic_hook::build_default_activation_with_state_home;
use std::fs;

use super::{git_init, make_executable, temp_root};

#[test]
fn default_activation_uses_state_home_runtime_provider_receipt() {
    let root = temp_root("state-home-runtime-provider");
    let state_home = root.join(".asp-state-home");
    git_init(&root);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"state-home-runtime-provider\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust candidate");
    install_state_home_provider(&state_home, "rust", "asp-rust", "rs-harness");

    let activation =
        build_default_activation_with_state_home(&root, &state_home).expect("build activation");
    let rust = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activated from State Home runtime bin");

    assert_eq!(rust.binary, "rs-harness");
    assert!(
        rust.provider_command_prefix.is_empty(),
        "State Home v1 activation must persist only the logical provider basename"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_resolves_nested_provider_project_entries() {
    let root = temp_root("nested-provider-project-entry");
    let state_home = root.join(".asp-state-home");
    git_init(&root);
    let package_root = root.join("packages").join("python");
    fs::create_dir_all(package_root.join("src")).expect("create nested Python source root");
    fs::write(
        package_root.join("pyproject.toml"),
        "[project]\nname = \"nested-provider-project-entry\"\nversion = \"0.1.0\"\n",
    )
    .expect("write nested pyproject.toml");
    fs::write(
        package_root.join("src").join("fixture.py"),
        "def fixture():\n    return 1\n",
    )
    .expect("write nested Python candidate");
    install_state_home_provider(&state_home, "python", "asp-python", "py-harness");

    let activation = build_default_activation_with_state_home(&root, &state_home)
        .expect("build nested provider activation");
    let python = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("nested Python provider activated");

    assert_eq!(
        python.coverage.package_roots,
        Vec::<String>::new(),
        "provider package roots must be rebased to the ASP workspace"
    );
    assert_eq!(
        python.coverage.config_files,
        ["pyproject.toml"],
        "provider project entry must be rebased to the ASP workspace"
    );
    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_rejects_project_relative_provider_override() {
    let root = temp_root("reject-project-relative-provider");
    let state_home = root.join(".asp-state-home");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"reject-project-relative-provider\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    write_agent_config(
        &root,
        "[providers.rust]\nbinary = \".bin/custom-rs-harness\"\n",
    );

    let error = build_default_activation_with_state_home(&root, &state_home)
        .expect_err("project-relative provider override must fail closed");
    assert!(
        error.contains("binary must be a logical basename resolved under State Home runtime/bin"),
        "{error}"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_rejects_absolute_provider_override() {
    let root = temp_root("reject-absolute-provider");
    let state_home = root.join(".asp-state-home");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"reject-absolute-provider\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    write_agent_config(
        &root,
        &format!(
            "[providers.rust]\nbinary = \"{}\"\n",
            root.join("custom-rs-harness").display()
        ),
    );

    let error = build_default_activation_with_state_home(&root, &state_home)
        .expect_err("absolute provider override must fail closed");
    assert!(
        error.contains("binary must be a logical basename resolved under State Home runtime/bin"),
        "{error}"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn document_language_flags_do_not_create_executable_activation_entries() {
    let root = temp_root("document-provider-disable");
    let state_home = root.join(".asp-state-home");
    git_init(&root);
    fs::write(root.join("README.md"), "# fixture\n").expect("write Markdown candidate");
    install_state_home_provider(&state_home, "rust", "asp-rust", "rs-harness");
    write_agent_config(
        &root,
        r#"[providers.typescript]
enabled = false

[providers.python]
enabled = false

[providers.julia]
enabled = false

[providers.gerbil-scheme]
enabled = false

[providers.org]
enabled = false
"#,
    );

    let activation =
        build_default_activation_with_state_home(&root, &state_home).expect("build activation");

    assert!(
        !activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "org")
    );
    assert!(
        !activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "md"),
        "document providers belong to the Hook policy projection, not executable activation"
    );
    assert!(
        activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "rust")
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn top_level_asp_toml_no_longer_configures_provider_activation() {
    let root = temp_root("top-level-ignored");
    let state_home = root.join(".asp-state-home");
    git_init(&root);
    fs::write(root.join("README.md"), "# fixture\n").expect("write Markdown candidate");
    fs::write(root.join("fixture.org"), "* Fixture\n").expect("write Org candidate");
    install_state_home_provider(&state_home, "rust", "asp-rust", "rs-harness");
    fs::write(root.join("asp.toml"), "[providers.rust]\nenabled = false\n")
        .expect("write ignored top-level asp.toml");

    let activation =
        build_default_activation_with_state_home(&root, &state_home).expect("build activation");

    assert!(
        activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "rust"),
        "build_default_activation must ignore top-level asp.toml; .agents/asp.toml is the only project provider config"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

pub(crate) fn install_state_home_provider(
    state_home: &std::path::Path,
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> std::path::PathBuf {
    let provider_bin = state_home.join("runtime").join("bin").join(binary);
    fs::create_dir_all(provider_bin.parent().expect("provider bin parent"))
        .expect("create State Home runtime bin");
    let project_contract = match language_id {
        "rust" => Some(("Cargo.toml", ".rs")),
        "typescript" => Some(("package.json", ".ts")),
        "python" => Some(("pyproject.toml", ".py")),
        "julia" => Some(("Project.toml", ".jl")),
        "gerbil-scheme" => Some(("gerbil.pkg", ".ss")),
        "org" | "md" => None,
        other => panic!("unsupported provider fixture language: {other}"),
    };
    let script = project_contract.map_or_else(
        || "#!/bin/sh\nexit 0\n".to_string(),
        |(project_entry, extension)| {
            format!(
                r#"#!/usr/bin/env python3
import json
import sys

if sys.argv[1:] != ["project-resolution-stdin"]:
    raise SystemExit(64)
request = json.load(sys.stdin)
generation = request["candidateGeneration"]["digest"]
scope = {{
    "schemaId": "agent.semantic-protocols.project-resolution",
    "schemaVersion": "1",
    "state": "resolved",
    "completeness": "exact",
    "languageId": "{language_id}",
    "providerId": "{provider_id}",
    "parserId": "fixture.package-manager",
    "candidateGenerationDigest": generation,
    "projectEntry": "{project_entry}",
    "packageGraph": {{
        "schemaId": "agent.semantic-protocols.language-package-graph",
        "schemaVersion": "1",
        "languageId": "{language_id}",
        "providerId": "{provider_id}",
        "projectEntry": "{project_entry}",
        "parserId": "fixture.package-manager",
        "manifests": [{{"path": "{project_entry}", "kind": "fixture-manifest", "digest": "fixture-manifest"}}],
        "lockfiles": [],
        "packages": [{{
            "packageId": "fixture",
            "name": "fixture",
            "manifestPath": "{project_entry}",
            "root": ".",
            "workspaceMember": True,
            "targets": [{{
                "targetId": "fixture:source",
                "kind": "source",
                "name": "fixture",
                "explicit": True,
                "sourceRoots": ["src"],
                "entrypoints": [],
                "generatedRoots": []
            }}]
        }}],
        "internalDependencyEdges": [],
        "externalDependencies": [],
        "unresolved": []
    }},
    "sourceScopes": [{{
        "scopeId": "fixture:source",
        "packageId": "fixture",
        "targetId": "fixture:source",
        "roots": ["src"],
        "explicitPaths": [],
        "extensions": ["{extension}"],
        "includeAuthority": "package-manager",
        "exclusions": []
    }}],
    "conflicts": [],
    "metrics": {{
        "parsedManifestCount": 1,
        "parsedLockfileCount": 0,
        "affectedPackageCount": 1,
        "fullWorkspaceReads": 0,
        "fullManifestReparses": 0,
        "dbOpens": 0,
        "elapsedMicros": 1
    }}
}}
json.dump({{
    "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
    "schemaVersion": "1",
    "state": "resolved",
    "languageId": "{language_id}",
    "providerId": "{provider_id}",
    "scope": scope
}}, sys.stdout, separators=(",", ":"), sort_keys=True)
sys.stdout.write("\n")
"#
            )
        },
    );
    fs::write(&provider_bin, script).expect("write provider bin");
    make_executable(&provider_bin);

    let entrypoint_digest = agent_semantic_content_identity::file_content_digest_v1(&provider_bin)
        .expect("provider content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&provider_bin)
            .expect("provider metadata digest");
    let lock_dir = agent_semantic_runtime::provider_receipt_dir(&state_home);
    fs::create_dir_all(&lock_dir).expect("create provider lock dir");
    fs::write(
        lock_dir.join(format!("{language_id}.lock.toml")),
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{metadata_digest}\"\n",
            provider_bin.display()
        ),
    )
    .expect("write provider install receipt");
    fs::canonicalize(&provider_bin).unwrap_or(provider_bin)
}

fn write_agent_config(root: &std::path::Path, contents: &str) {
    let config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    fs::write(&config_path, contents).expect("write .agents/asp.toml");
}
