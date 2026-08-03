use tokio::sync::mpsc;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum RuntimeServerEvent {
    ConnectionRejected(String),
    ConnectionTaskFailed(String),
    WorkspaceGenerationMaterializationObserved {
        workspace_identity: String,
        build_mode: String,
        owner_count: usize,
        owner_source_bytes: u64,
        selector_count: usize,
        relation_count: usize,
    },
    WorkspaceGenerationResidentPublished {
        workspace_identity: String,
        build_mode: String,
        generation_digest: String,
        elapsed_micros: u64,
    },
    WorkspaceGenerationRestoreFailed {
        workspace_identity: String,
        error: String,
    },
}

pub(crate) fn publish_event(
    events: Option<&mpsc::UnboundedSender<RuntimeServerEvent>>,
    event: RuntimeServerEvent,
) {
    if let Some(events) = events {
        let _ = events.send(event);
    }
}
