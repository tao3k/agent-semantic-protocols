//! Runtime Server identity successor handoff and its ordered test contract.

use agent_semantic_client_db::runtime_server_control::cleanup_endpoint;

pub(super) struct RuntimeIdentityHandoffCoordinator<'a> {
    state_home: &'a std::path::Path,
    endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
}

#[cfg(test)]
trait RuntimeIdentityHandoff {
    fn retire(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn cleanup(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn admit(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn publish(
        &mut self,
        success: bool,
        error: Option<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>>;
}

#[cfg(test)]
async fn execute_runtime_identity_handoff<T: RuntimeIdentityHandoff>(
    mut adapter: T,
) -> Result<(), String> {
    adapter.retire().await?;
    adapter.cleanup().await?;
    match adapter.admit().await {
        Ok(()) => {
            adapter.publish(true, None).await;
            Ok(())
        }
        Err(error) => {
            let typed =
                format!("Runtime Server identity successor failed readiness admission: {error}");
            adapter.publish(false, Some(typed.clone())).await;
            Err(typed)
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_identity_handoff.rs"]
mod runtime_identity_handoff_tests;

impl<'a> RuntimeIdentityHandoffCoordinator<'a> {
    pub(super) fn new(
        state_home: &'a std::path::Path,
        endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
    ) -> Self {
        Self {
            state_home,
            endpoint,
        }
    }

    pub(super) async fn retire(&self) -> Result<(), String> {
        agent_semantic_client_db::runtime_server_supervisor::retire_runtime_server_owner_for_handoff(
            self.state_home,
            self.endpoint,
        )
        .await
    }

    pub(super) async fn cleanup(&self) -> Result<(), String> {
        cleanup_endpoint(self.state_home, self.endpoint).await
    }

    pub(super) async fn admit_successor(
        &self,
        event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
        serving_digest: Option<
            &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
        >,
    ) -> Result<(), String> {
        crate::server::runtime_server_wire_adapter::ensure_healthy_runtime_server_for_activation_event(
            self.state_home,
            event,
            serving_digest,
        )
        .await
        .map(|_| ())
    }
}
