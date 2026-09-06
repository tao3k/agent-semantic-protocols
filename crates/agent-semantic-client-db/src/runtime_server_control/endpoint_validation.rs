use std::path::Path;

use super::endpoint_identity::runtime_server_runtime_base;
use super::model::RuntimeServerEndpoint;

pub fn validate_runtime_server_endpoint_for_state_home(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    endpoint.validate()?;
    let runtime_base = runtime_server_runtime_base(state_home)?;
    let expected_status_memory =
        super::endpoint_identity::runtime_server_status_memory_path_for_identity(
            &runtime_base,
            endpoint.owner_epoch,
            &endpoint.binding_token,
            endpoint.runtime_binary_identity.content_digest(),
        );
    if Path::new(&endpoint.status_memory_path) != expected_status_memory {
        return Err(format!(
            "Runtime Server endpoint State Home or binding identity mismatch: stateHome={} ownerEpoch={} expectedStatusMemory={} observedStatusMemory={}",
            state_home.display(),
            endpoint.owner_epoch,
            expected_status_memory.display(),
            endpoint.status_memory_path,
        ));
    }
    Ok(())
}
