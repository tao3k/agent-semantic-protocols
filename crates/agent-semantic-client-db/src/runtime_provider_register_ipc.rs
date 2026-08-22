use crate::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_provider_protocol::ProviderRegisterRequest;
use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::sync::watch;

pub(crate) async fn serve_provider_register_stream(
    mut stream: UnixStream,
    register: Arc<RuntimeProviderRegister>,
    mut drain: watch::Receiver<bool>,
) -> Result<(), String> {
    loop {
        let request = tokio::select! {
            changed = drain.changed() => {
                match changed {
                    Ok(()) if *drain.borrow() => return Ok(()),
                    Ok(()) => continue,
                    Err(_) => return Ok(()),
                }
            }
            request = crate::workspace_db_ipc::read_optional_frame::<ProviderRegisterRequest>(&mut stream) => {
                request?
            }
        };
        let Some(request) = request else {
            return Ok(());
        };
        let response = register.apply(request).await?;
        crate::workspace_db_ipc::write_frame(&mut stream, &response).await?;
    }
}
