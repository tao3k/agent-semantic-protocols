use std::path::Path;

pub(super) fn admit_embedded_hook_config() -> Result<(), String> {
    agent_semantic_config::default_hook_client_config_file()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "ASP binary/config publication admission failed before artifact switch: {error}"
            )
        })
}

/// Publish the Hook matcher contract embedded in the installing executable.
///
/// Binary and matcher config are one compatibility generation.  Keeping this
/// publication in the canonical binary installer means a stale matcher can
/// never block the command that repairs the pair; Runtime Server availability
/// is deliberately not part of this local recovery edge.
pub(super) fn publish_embedded_hook_config(protocol_home: &Path) -> Result<&'static str, String> {
    let path = protocol_home.join("hooks/config.toml");
    super::managed_hook_config::materialize(&path)
        .map(super::managed_hook_config::ManagedHookConfigStatus::as_str)
        .map_err(|error| {
            format!(
                "ASP binary/config publication failed for {} after binary switch: {error}",
                path.display()
            )
        })
}

#[cfg(test)]
#[path = "../../tests/unit/install_binary_config_admission.rs"]
mod tests;
