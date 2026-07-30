use agent_semantic_hook::build_default_activation;
use std::env;
use std::ffi::OsString;
use std::fs;

use super::{git_init, make_executable, temp_root};

pub(crate) struct StateHomeEnvGuard {
    previous: Option<OsString>,
}

impl StateHomeEnvGuard {
    pub(crate) fn set(state_home: &std::path::Path) -> Self {
        let previous = env::var_os("ASP_STATE_HOME");
        unsafe {
            env::set_var("ASP_STATE_HOME", state_home);
        }
        Self { previous }
    }
}

impl Drop for StateHomeEnvGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                env::set_var("ASP_STATE_HOME", previous);
            } else {
                env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}

#[test]
fn default_activation_uses_state_home_runtime_provider_receipt() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("state-home-runtime-provider");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
    git_init(&root);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"state-home-runtime-provider\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust candidate");
    install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");

    let activation = build_default_activation(&root).expect("build activation");
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
fn default_activation_rejects_project_relative_provider_override() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("reject-project-relative-provider");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
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

    let error = build_default_activation(&root)
        .expect_err("project-relative provider override must fail closed");
    assert!(
        error.contains("binary must be a logical basename resolved under State Home runtime/bin"),
        "{error}"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_rejects_absolute_provider_override() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("reject-absolute-provider");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
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

    let error =
        build_default_activation(&root).expect_err("absolute provider override must fail closed");
    assert!(
        error.contains("binary must be a logical basename resolved under State Home runtime/bin"),
        "{error}"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn asp_toml_can_disable_document_language_hook_activation() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("document-provider-disable");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
    git_init(&root);
    fs::write(root.join("README.md"), "# fixture\n").expect("write Markdown candidate");
    install_state_home_provider(&state_home, "md", "orgize", "orgize");
    write_agent_config(
        &root,
        r#"[providers.rust]
enabled = false

[providers.typescript]
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
    assert_eq!(md.binary, "orgize");
    assert_eq!(md.execution.as_str(), "external-process");
    assert!(
        md.coverage.package_roots.is_empty(),
        "document resolution must not invent a package-manager root"
    );
    assert!(
        md.provider_command_prefix.is_empty(),
        "document activation must not persist the receipt-resolved provider path"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn top_level_asp_toml_no_longer_configures_provider_activation() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("top-level-ignored");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
    git_init(&root);
    fs::write(root.join("README.md"), "# fixture\n").expect("write Markdown candidate");
    fs::write(root.join("fixture.org"), "* Fixture\n").expect("write Org candidate");
    install_state_home_provider(&state_home, "org", "orgize", "orgize");
    install_state_home_provider(&state_home, "md", "orgize", "orgize");
    fs::write(root.join("asp.toml"), "[providers.org]\nenabled = false\n")
        .expect("write ignored top-level asp.toml");

    let activation = build_default_activation(&root).expect("build activation");

    assert!(
        activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "org"),
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
                r#"#!/bin/sh
if [ "$1" != "project-resolution-stdin" ]; then
  exit 64
fi
printf '%s\n' '{{"schemaId":"agent.semantic-protocols.provider-project-resolution-response","schemaVersion":"1","state":"resolved","languageId":"{language_id}","providerId":"{provider_id}","resolution":{{"schemaId":"agent.semantic-protocols.project-resolution","schemaVersion":"1","state":"resolved","completeness":"exact","projectIdentity":{{"projectEntry":"{project_entry}"}},"resolvedSourceScopes":[{{"roots":["src"],"extensions":["{extension}"],"exclusions":[]}}],"resolutionGeneration":"fixture:v1"}}}}'
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
