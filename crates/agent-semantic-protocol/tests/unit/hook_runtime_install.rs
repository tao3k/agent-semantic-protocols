use super::{CodexPluginScope, parse_codex_plugin_install_args};

#[test]
fn clap_rejects_global_and_project_scope_together() {
    let args = vec![
        "--codex".to_string(),
        "--global".to_string(),
        "--project".to_string(),
        ".".to_string(),
    ];
    let error = parse_codex_plugin_install_args(&args).expect_err("ambiguous scope");
    assert!(error.contains("--global"), "{error}");
    assert!(error.contains("--project"), "{error}");
}

#[test]
fn clap_defaults_plugin_install_to_global_scope() {
    let args = vec!["--codex".to_string(), ".".to_string()];
    let request = parse_codex_plugin_install_args(&args).expect("default global scope");
    assert!(matches!(request.scope, CodexPluginScope::Global));
}

#[test]
fn clap_parses_global_scope_without_project_pollution() {
    let args = vec![
        "--codex".to_string(),
        "--global".to_string(),
        ".".to_string(),
    ];
    let request = parse_codex_plugin_install_args(&args).expect("global scope");
    assert!(matches!(request.scope, CodexPluginScope::Global));
}

#[test]
fn clap_parses_project_scope_explicitly() {
    let args = vec![
        "--codex".to_string(),
        "--project".to_string(),
        ".".to_string(),
    ];
    let request = parse_codex_plugin_install_args(&args).expect("project scope");
    assert!(matches!(request.scope, CodexPluginScope::Project));
}
