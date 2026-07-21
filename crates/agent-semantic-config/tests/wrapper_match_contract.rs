use agent_semantic_config::HookClientConfigFile;

#[test]
fn wrapper_match_defaults_to_enable_and_invalid_values_fail_closed() {
    let config = HookClientConfigFile::default();
    assert_eq!(format!("{:?}", config.wrapper_match), "Enable");

    let error = toml::from_str::<HookClientConfigFile>("wrapper_match = \"disable\"").unwrap_err();
    let message = error.to_string();
    assert!(message.contains("disable"), "error={message}");
    assert!(message.contains("enable"), "error={message}");
}
