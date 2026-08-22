use std::path::Path;

use super::endpoint_identity::runtime_server_runtime_base;
use super::model::RuntimeServerEndpoint;

const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

pub fn validate_runtime_server_endpoint_for_state_home(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    endpoint.validate()?;
    let runtime_base = runtime_server_runtime_base(state_home)?;
    let digest = blake3::hash(
        format!(
            "{}\0{}\0{}",
            endpoint.owner_epoch,
            endpoint.binding_token,
            endpoint.runtime_binary_identity.value()
        )
        .as_bytes(),
    )
    .to_hex();
    let expected_socket = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    let expected_data_socket = runtime_base.join(format!("r-{}.data.sock", &digest[..16]));
    let expected_provider_socket = super::provider_endpoint::provider_plane_socket_path(
        &runtime_base,
        &digest,
        MAX_UNIX_SOCKET_PATH_BYTES,
    )?;
    let expected_status_memory = runtime_base.join("status.v1.memory");
    if Path::new(&endpoint.socket_path) != expected_socket
        || Path::new(&endpoint.data_plane_socket_path) != expected_data_socket
        || Path::new(&endpoint.provider_plane_socket_path) != expected_provider_socket
        || Path::new(&endpoint.status_memory_path) != expected_status_memory
    {
        return Err(format!(
            "Runtime Server endpoint State Home or binding identity mismatch: stateHome={} ownerEpoch={}",
            state_home.display(),
            endpoint.owner_epoch
        ));
    }
    Ok(())
}
