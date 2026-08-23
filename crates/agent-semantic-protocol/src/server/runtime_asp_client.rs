//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientProtocolHttpService,
};

type ClientRequestKey = (String, String, String);

#[derive(Clone)]
pub(super) struct RuntimeAspClientDispatcher {
    runtime_search_service: RuntimeSearchServiceHandle,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    provider_register: Arc<RuntimeProviderRegister>,
    cancellations: Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
}

impl RuntimeAspClientDispatcher {
    fn new(
        runtime_search_service: RuntimeSearchServiceHandle,
        workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
        provider_register: Arc<RuntimeProviderRegister>,
    ) -> Self {
        Self {
            runtime_search_service,
            workspace_registry,
            provider_register,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub(super) async fn bind_http_listener() -> Result<(tokio::net::TcpListener, String), String> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("failed to bind ASP Client Protocol HTTP endpoint: {error}"))?;
    let endpoint = format!(
        "http://{}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to resolve ASP Client Protocol endpoint: {error}"))?
    );
    Ok((listener, endpoint))
}

pub(super) fn build_http_service(
    runtime_search_service: RuntimeSearchServiceHandle,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    provider_register: Arc<RuntimeProviderRegister>,
) -> Arc<AspClientProtocolHttpService<RuntimeAspClientDispatcher>> {
    let catalog_workspace_registry = Arc::clone(&workspace_registry);
    let catalog_provider_register = Arc::clone(&provider_register);
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        runtime_search_service,
        workspace_registry,
        provider_register,
    ));
    Arc::new(AspClientProtocolHttpService::new(
        dispatcher,
        move |workspace_identity| {
            let (_, generation_digest) =
                catalog_workspace_registry.unique_resident_scope(workspace_identity)?;
            catalog_provider_register.client_protocol_catalog(
                generation_digest,
                vec![agent_semantic_client_protocol::ClientTransport::HttpJson],
            )
        },
    ))
}

impl AspClientDispatcher for RuntimeAspClientDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        let key = (
            request.workspace_identity.clone(),
            request.session_id.clone(),
            request.request_id.clone(),
        );
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        let admitted = {
            let mut cancellations = self
                .cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned");
            match cancellations.entry(key.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(cancel);
                    true
                }
                Entry::Occupied(_) => false,
            }
        };
        if !admitted {
            return Box::pin(async {
                Err(AspClientDispatchError {
                    reason_kind: "client-request-id-conflict".to_owned(),
                    message: "requestId is already in flight for this client session".to_owned(),
                })
            });
        }

        let runtime_search_service = self.runtime_search_service.clone();
        let workspace_registry = Arc::clone(&self.workspace_registry);
        let provider_register = Arc::clone(&self.provider_register);
        let cancellations = Arc::clone(&self.cancellations);
        Box::pin(async move {
            let operation = async {
                let (project_root, _) =
                    workspace_registry.unique_resident_scope(&request.workspace_identity)?;
                let (language_id, operation, _) =
                    provider_register.resolve_client_method(&request.method)?;
                runtime_search_service
                    .provider_runtime(project_root.clone(), language_id.clone())
                    .await?;
                runtime_search_service
                    .provider_runtime_await_ready(project_root.clone(), language_id.clone())
                    .await?;
                let payload = serde_json::to_vec(&request.params)
                    .map_err(|error| format!("encode ASP client request params: {error}"))?;
                let response = runtime_search_service
                    .provider_operation(project_root, language_id, operation, payload)
                    .await?;
                serde_json::from_slice(&response)
                    .map_err(|error| format!("decode ASP client provider response: {error}"))
            };
            let result = tokio::select! {
                result = operation => result.map_err(|message| AspClientDispatchError {
                    reason_kind: "client-method-dispatch-failed".to_owned(),
                    message,
                }),
                changed = cancelled.changed() => {
                    let _ = changed;
                    Err(AspClientDispatchError {
                        reason_kind: "client-request-cancelled".to_owned(),
                        message: "client request was cancelled".to_owned(),
                    })
                }
            };
            cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned")
                .remove(&key);
            result
        })
    }

    fn cancel(&self, workspace_identity: &str, session_id: &str, request_id: &str) -> bool {
        self.cancellations
            .lock()
            .expect("ASP Client Protocol cancellation registry poisoned")
            .remove(&(
                workspace_identity.to_owned(),
                session_id.to_owned(),
                request_id.to_owned(),
            ))
            .is_some_and(|cancel| cancel.send(true).is_ok())
    }
}
