use std::env;
use std::path::Path;

use crate::provider_command::support::{
    asp_command, home_local_bin, make_executable, provider, temp_project_root, write_activation_to,
    write_provider_bin_config,
};

#[test]
fn runtime_profile_command_prefix_overrides_home_local_binary() {
    let root = temp_project_root("provider-prefix-facade");
    std::fs::remove_dir_all(root.join(".git")).expect("remove temp git marker");
    let cache_home = root.join(".cache-home");
    let home = root.join("home");
    let activation_path = cache_home.join("agent-semantic-protocol/hooks/activation.json");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let profile_wrapper = bin_dir.join("provider-wrapper");
    std::fs::write(
        &profile_wrapper,
        r#"#!/bin/sh
printf 'profile args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
printf 'cache=%s
' "$PRJ_CACHE_HOME"
printf 'runtime=%s
' "$ASP_RUNTIME_BIN_DIR"
printf 'path0=%s
' "${PATH%%:*}"
printf 'renderer=%s
' "$SEMANTIC_AGENT_PROTOCOL_BIN"
"#,
    )
    .expect("write profile provider wrapper");
    make_executable(&profile_wrapper);
    let home_bin = home_local_bin(&root);
    let wrapper_path = home_bin.join("rs-harness");
    std::fs::create_dir_all(&home_bin).expect("create home local bin");
    std::fs::write(
        &wrapper_path,
        r#"#!/bin/sh
printf 'home-local args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
printf 'cache=%s
' "$PRJ_CACHE_HOME"
printf 'runtime=%s
' "$ASP_RUNTIME_BIN_DIR"
printf 'path0=%s
' "${PATH%%:*}"
printf 'renderer=%s
' "$SEMANTIC_AGENT_PROTOCOL_BIN"
"#,
    )
    .expect("write provider wrapper");
    make_executable(&wrapper_path);
    write_activation_to(
        &root,
        &activation_path,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .env("HOME", &home)
        .env("PATH", &bin_dir)
        .env_remove("SEMANTIC_AGENT_PROTOCOL_BIN")
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cache_home = std::fs::canonicalize(&cache_home).unwrap_or_else(|_| cache_home.clone());
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("profile args=[rs-harness][guide][.]"),
        "{stdout}"
    );
    assert!(!stdout.contains("home-local args="), "{stdout}");
    assert!(
        stdout.contains(&format!("cache={}\n", cache_home.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("runtime={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("path0={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("renderer={}\n", env!("CARGO_BIN_EXE_asp"))),
        "{stdout}"
    );

    let nested_root = root.join("languages/rust-lang-project-harness");
    std::fs::create_dir_all(&nested_root).expect("create nested root");
    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .env("HOME", &home)
        .env("PATH", &bin_dir)
        .args(["rust", "guide", "languages/rust-lang-project-harness"])
        .output()
        .expect("run asp rust guide nested root");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("profile args=[rs-harness][guide][languages/rust-lang-project-harness]"),
        "{stdout}"
    );
    assert!(!stdout.contains("home-local args="), "{stdout}");
    assert!(
        stdout.contains(&format!("cache={}\n", cache_home.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("runtime={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("path0={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("renderer={}\n", env!("CARGO_BIN_EXE_asp"))),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn configured_provider_bin_overrides_home_local_binary() {
    let root = temp_project_root("provider-bin-config-overrides-home-local");
    std::fs::remove_dir_all(root.join(".git")).expect("remove temp git marker");
    let cache_home = root.join(".cache-home");
    let home = root.join("home");
    let activation_path = cache_home.join("agent-semantic-protocol/hooks/activation.json");
    let configured_bin_dir = root.join("configured-bin");
    let configured_provider = configured_bin_dir.join("configured-rs-harness");
    std::fs::create_dir_all(&configured_bin_dir).expect("create configured bin dir");
    std::fs::write(
        &configured_provider,
        r#"#!/bin/sh
printf 'configured args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
"#,
    )
    .expect("write configured provider");
    make_executable(&configured_provider);

    let home_bin = home_local_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create home local bin");
    let home_provider = home_bin.join("rs-harness");
    std::fs::write(
        &home_provider,
        r#"#!/bin/sh
printf 'home-local args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
"#,
    )
    .expect("write home local provider");
    make_executable(&home_provider);

    write_provider_bin_config(&root, "rust", &configured_provider);
    write_activation_to(&root, &activation_path, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .env("HOME", &home)
        .env("PATH", root.join("empty-path"))
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide with configured provider bin");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("configured args=[guide][.]"), "{stdout}");
    assert!(!stdout.contains("home-local args="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn configured_provider_bin_name_uses_home_local_gslph() {
    let root = temp_project_root("provider-bin-name-home-local-gslph");
    std::fs::remove_dir_all(root.join(".git")).expect("remove temp git marker");
    let cache_home = root.join(".cache-home");
    let home = root.join("home");
    let activation_path = cache_home.join("agent-semantic-protocol/hooks/activation.json");
    let home_bin = home_local_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create home local bin");
    let home_provider = home_bin.join("gslph");
    std::fs::write(
        &home_provider,
        r#"#!/bin/sh
printf 'home-gslph args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
"#,
    )
    .expect("write home local provider");
    make_executable(&home_provider);

    write_provider_bin_config(&root, "rust", Path::new("gslph"));
    write_activation_to(&root, &activation_path, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .env("HOME", &home)
        .env("PATH", root.join("empty-path"))
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide with configured gslph bin");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("home-gslph args=[guide][.]"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
