use crate::rust_harness_activation::support::write_fake_provider_binary;

use super::support::{git_project_root, protocol_command};

#[test]
fn cli_install_refuses_protocol_bin_dir_outside_path() {
    let root = git_project_root("install-protocol-bin-path");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let output = protocol_command()
        .env("PATH", &provider_path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SEMANTIC_AGENT_BIN_DIR="), "{stderr}");
    assert!(stderr.contains("is not present in PATH"), "{stderr}");
    assert!(!root.join(".codex/config.toml").exists());
    assert!(!protocol_bin_dir.join("asp").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_refuses_to_overwrite_invalid_codex_toml() {
    let root = git_project_root("install-invalid-toml");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(&config_path, "unified_exec = \"unterminated\n").expect("write invalid config");

    let output = protocol_command()
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("refusing to write invalid Codex config TOML")
    );
    let config = std::fs::read_to_string(&config_path).expect("preserved config");
    assert_eq!(config, "unified_exec = \"unterminated\n");
    assert!(!config.contains("# BEGIN agent-semantic-protocol agent hooks"));
    let _ = std::fs::remove_dir_all(&root);
}
