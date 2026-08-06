use super::subagent_model_arg;

#[test]
fn codex_plugin_install_cannot_override_registry_owned_models() {
    assert!(subagent_model_arg("codex", None).is_err());
    assert!(subagent_model_arg("codex", Some("gpt-5.6-luna")).is_err());
}
