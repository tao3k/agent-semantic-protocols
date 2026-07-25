use agent_semantic_hook::build_default_activation;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::sync::Mutex;

use super::{git_init, make_executable, temp_root};

pub(crate) static STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct StateHomeEnvGuard {
    previous: Option<OsString>,
}

impl StateHomeEnvGuard {
    pub(crate) fn set(home: &std::path::Path) -> Self {
        let previous = env::var_os("ASP_STATE_HOME");
        unsafe {
            env::set_var("ASP_STATE_HOME", home);
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
    let _state_home_lock = STATE_HOME_ENV_LOCK
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
    let provider_bin = install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");

    let activation = build_default_activation(&root).expect("build activation");
    let rust = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activated from State Home runtime bin");

    assert_eq!(rust.binary, "rs-harness");
    assert_eq!(
        rust.provider_command_prefix,
        vec![provider_bin.display().to_string()],
        "State Home v1 provider receipt must own the activated command prefix"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn default_activation_rejects_project_relative_provider_override() {
    let _state_home_lock = STATE_HOME_ENV_LOCK
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
    let _state_home_lock = STATE_HOME_ENV_LOCK
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
    let _state_home_lock = STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("document-provider-disable");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
    let orgize_bin = install_state_home_provider(&state_home, "md", "orgize", "orgize");
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
        md.provider_command_prefix
            .first()
            .is_some_and(|command| command == &orgize_bin.display().to_string()),
        "document provider should route through the State Home orgize receipt: {:?}",
        md.provider_command_prefix
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn top_level_asp_toml_no_longer_configures_provider_activation() {
    let _state_home_lock = STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("top-level-ignored");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = StateHomeEnvGuard::set(&state_home);
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
    fs::write(&provider_bin, "#!/bin/sh\nexit 0\n").expect("write provider bin");
    make_executable(&provider_bin);

    let entrypoint_digest = agent_semantic_content_identity::file_content_digest_v1(&provider_bin)
        .expect("provider content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&provider_bin)
            .expect("provider metadata digest");
    let lock_dir = state_home.join("runtime").join("provider-locks");
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
