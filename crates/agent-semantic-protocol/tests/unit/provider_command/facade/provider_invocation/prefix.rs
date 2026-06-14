use std::env;

use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_cache_source_fixture,
};

#[test]
fn provider_command_prefix_is_used_as_full_invocation_prefix() {
    let root = temp_project_root("provider-prefix-facade");
    write_cache_source_fixture(&root);
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
    write_activation(
        &root,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .env_remove("SEMANTIC_AGENT_PROTOCOL_BIN")
        .args(["rust", "query", "src/lib.rs", "."])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let cache_home = canonical_root.join(".cache");
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "wrapper args=[rs-harness][query][src/lib.rs]\ncache={}\nruntime={}\npath0={}\nrenderer={}\n",
            cache_home.display(),
            runtime_bin.display(),
            runtime_bin.display(),
            env!("CARGO_BIN_EXE_asp")
        )
    );

    let nested_root = root.join("languages/rust-lang-project-harness");
    write_activation(
        &nested_root,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );
    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "check",
            "--changed",
            "languages/rust-lang-project-harness",
        ])
        .output()
        .expect("run asp rust check nested root");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "wrapper args=[rs-harness][check][--changed]\ncache={}\nruntime={}\npath0={}\nrenderer={}\n",
            cache_home.display(),
            runtime_bin.display(),
            runtime_bin.display(),
            env!("CARGO_BIN_EXE_asp")
        )
    );
    let _ = std::fs::remove_dir_all(root);
}
