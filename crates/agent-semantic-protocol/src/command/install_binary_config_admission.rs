pub(super) fn admit_embedded_hook_config() -> Result<(), String> {
    agent_semantic_config::default_hook_client_config_file()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "ASP binary/config publication admission failed before artifact switch: {error}"
            )
        })
}
