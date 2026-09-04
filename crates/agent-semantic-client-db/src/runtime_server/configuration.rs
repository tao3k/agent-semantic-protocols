//! Construction-time Runtime Server dependency injection.
//!
//! These builders deliberately live apart from `core`: they assemble daemon
//! dependencies once, while `core` owns the serving loop and its lifecycle.

use std::sync::Arc;

use super::{AspPythonGraphsStatusHandle, RuntimeServer};

impl RuntimeServer {
    pub fn with_event_sender(
        mut self,
        events: crate::runtime_server_observability::RuntimeServerEventPublisher,
    ) -> Self {
        self.events = Some(events);
        self
    }

    pub fn with_runtime_telemetry_sender(
        mut self,
        sender: crate::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        self.telemetry_sender = Some(sender);
        self
    }

    pub fn with_runtime_search_service(
        mut self,
        service: crate::runtime_search_service::RuntimeSearchServiceHandle,
    ) -> Self {
        self.runtime_search_service = Some(service);
        self
    }

    pub fn with_asp_python_graphs_status(mut self, status: AspPythonGraphsStatusHandle) -> Self {
        self.status_memory.set_asp_python_graphs(status.shared());
        self.asp_python_graphs_status = Some(status);
        self
    }

    /// Compose the server with an already-owned generation admission plane.
    pub fn with_workspace_generation_admission(
        mut self,
        admission: Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    ) -> Self {
        self.generation_admission = Some(admission);
        self
    }
}
