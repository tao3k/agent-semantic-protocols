use std::env;

use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation_to,
};

#[test]
fn provider_command_prefix_is_used_as_full_invocation_prefix() {
    let root = temp_project_root("provider-prefix-facade");
    std::fs::remove_dir_all(root.join(".git")).expect("remove temp git marker");
    let cache_home = root.join(".cache-home");
    let home = root.join("home");
    let home_local_bin = home.join(".local/bin");
    std::fs::create_dir_all(&home_local_bin).expect("create home local bin");
    let activation_path = cache_home.join("agent-semantic-protocol/hooks/activation.json");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let wrapper_path = bin_dir.join("provider-wrapper");
    std::fs::write(
        &wrapper_path,
        r#"#!/bin/sh
printf 'wrapper args='
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
        stdout.contains("wrapper args=[rs-harness][guide]"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("cache={}\n", cache_home.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("runtime={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("path0={}\n", home_local_bin.display())),
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
        stdout.contains("wrapper args=[rs-harness][guide]"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("cache={}\n", cache_home.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("runtime={}\n", runtime_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("path0={}\n", home_local_bin.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("renderer={}\n", env!("CARGO_BIN_EXE_asp"))),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
