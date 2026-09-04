use super::CodexPluginInstallOperation;
use super::CodexPluginSourceRootSource;
use super::parse_codex_plugin_install_args;

#[test]
fn clap_rejects_removed_project_scope() {
    let args = vec![
        "status".to_string(),
        "--codex".to_string(),
        "--project".to_string(),
        ".".to_string(),
    ];
    assert!(parse_codex_plugin_install_args(&args).is_err());
}

#[test]
fn clap_accepts_explicit_source_override_without_changing_global_scope() {
    let args = vec!["status".to_string(), "--codex".to_string(), ".".to_string()];
    let request = parse_codex_plugin_install_args(&args).expect("global plugin status");
    assert_eq!(request.operation, CodexPluginInstallOperation::Status);
    assert_eq!(
        request.source_root_source,
        CodexPluginSourceRootSource::ExplicitOverride
    );
}

#[test]
fn clap_rejects_removed_global_scope_flag() {
    let args = vec![
        "publish".to_string(),
        "--codex".to_string(),
        "--global".to_string(),
        ".".to_string(),
    ];
    assert!(parse_codex_plugin_install_args(&args).is_err());
}

#[test]
fn clap_requires_explicit_status_or_publish_operation() {
    let args = vec!["--codex".to_string(), ".".to_string()];
    assert!(parse_codex_plugin_install_args(&args).is_err());
}
