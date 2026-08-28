use super::parse_codex_plugin_install_args;

#[test]
fn clap_rejects_removed_project_scope() {
    let args = vec![
        "--codex".to_string(),
        "--project".to_string(),
        ".".to_string(),
    ];
    assert!(parse_codex_plugin_install_args(&args).is_err());
}

#[test]
fn clap_defaults_plugin_install_to_global_scope() {
    let args = vec!["--codex".to_string(), ".".to_string()];
    parse_codex_plugin_install_args(&args).expect("global install");
}

#[test]
fn clap_rejects_removed_global_scope_flag() {
    let args = vec![
        "--codex".to_string(),
        "--global".to_string(),
        ".".to_string(),
    ];
    assert!(parse_codex_plugin_install_args(&args).is_err());
}
