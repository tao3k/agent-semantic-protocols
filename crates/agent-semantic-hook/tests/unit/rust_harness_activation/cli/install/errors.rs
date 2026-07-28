use crate::rust_harness_activation::support::write_state_home_provider_binary;

use super::support::{
    codex_plugin_install_args, git_project_root, protocol_command, sync_test_state,
};

#[test]
fn cli_install_refuses_protocol_bin_dir_outside_path() {
    let root = git_project_root("install-protocol-bin-path");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "rust", "rs-harness", "rs-harness");
    sync_test_state(&root, &asp_state_home);
    let unrelated_bin_dir = root.join(".unrelated-bin");
    std::fs::create_dir_all(&unrelated_bin_dir).expect("create unrelated bin dir");
    write_real_asp_launcher(&unrelated_bin_dir);
    write_fake_codex_cli_in_dir(&unrelated_bin_dir);
    let protocol_bin_dir = root.join(".agent-bin");
    let output = protocol_command()
        .env("PATH", &unrelated_bin_dir)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", &codex_home)
        .args(codex_plugin_install_args(&root))
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
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "rust", "rs-harness", "rs-harness");
    sync_test_state(&root, &asp_state_home);
    let protocol_bin_dir = root.join(".agent-bin");
    write_real_asp_launcher(&protocol_bin_dir);
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(&config_path, "unified_exec = \"unterminated\n").expect("write invalid config");

    let output = protocol_command()
        .env("PATH", &protocol_bin_dir)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refusing to clean invalid Codex config TOML"),
        "{stderr}"
    );
    let config = std::fs::read_to_string(&config_path).expect("preserved config");
    assert_eq!(config, "unified_exec = \"unterminated\n");
    assert!(!config.contains("# BEGIN agent-semantic-protocol agent hooks"));
    let _ = std::fs::remove_dir_all(&root);
}
use crate::rust_harness_activation::cli::install::support::{
    write_fake_codex_cli_in_dir, write_real_asp_launcher,
};
