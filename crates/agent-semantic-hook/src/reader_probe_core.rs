//! Minimal Reader behavior observation shared by the Hook compiler and evaluator.

use serde_json::Value;

#[path = "reader_probe_runtime/mod.rs"]
mod runtime;

#[doc(hidden)]
pub fn diagnose_reader_probe(
    command_tokens: Vec<String>,
    subject: String,
    wrapped_command: bool,
    reader_behavior_patterns: Vec<Vec<String>>,
) -> Option<ReaderProbeObservation> {
    runtime::observe(&runtime::ReaderProbeRequest {
        command_tokens,
        subject,
        wrapped_command,
        reader_behavior_patterns,
        dynamic_cache_root: None,
    })
}

#[doc(hidden)]
pub fn diagnose_reader_probe_with_state_home(
    command_tokens: Vec<String>,
    subject: String,
    wrapped_command: bool,
    reader_behavior_patterns: Vec<Vec<String>>,
    state_home: &std::path::Path,
) -> Option<ReaderProbeObservation> {
    runtime::observe(&runtime::ReaderProbeRequest {
        command_tokens,
        subject,
        wrapped_command,
        reader_behavior_patterns,
        dynamic_cache_root: Some(
            agent_semantic_artifacts::StateHomeLayout::new(state_home)
                .cache()
                .reader_behavior(),
        ),
    })
}

pub(crate) const READER_PROBE_FIELD: &str = "_aspReaderProbe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReaderProbeAccess {
    Read,
    Unknown,
}

impl ReaderProbeAccess {
    const fn label(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderProbeObservation {
    pub subject: String,
    pub access: ReaderProbeAccess,
    pub backend: String,
    pub terminal: String,
    pub elapsed_micros: u64,
    pub probe_process_launched: bool,
    pub cleanup_verified: bool,
    pub cache_hit: bool,
    pub behavior_key: Option<String>,
}

pub fn bind_reader_probe_observation(
    payload: &mut Value,
    observation: Option<&ReaderProbeObservation>,
) -> Result<(), String> {
    let input_key = ["tool_input", "toolInput", "parameters", "input"]
        .into_iter()
        .find(|key| payload.get(*key).is_some())
        .ok_or_else(|| "Reader probe payload requires a Host tool input".to_owned())?;
    let input = payload
        .get_mut(input_key)
        .expect("Host tool input key was observed in the same payload");
    let object = input
        .as_object_mut()
        .ok_or_else(|| "Reader probe Host tool input must be an object".to_owned())?;
    object.remove(READER_PROBE_FIELD);
    if let Some(observation) = observation {
        object.insert(
            READER_PROBE_FIELD.to_owned(),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.reader-probe-observation",
                "schemaVersion": 1,
                "subject": observation.subject,
                "access": observation.access.label(),
                "accessMode": match observation.access {
                    ReaderProbeAccess::Read => "read-permission",
                    ReaderProbeAccess::Unknown => "unknown",
                },
                "backend": observation.backend,
                "permissionProbe": (observation.backend == "permission-differential")
                    .then_some("readable-vs-denied"),
                "terminal": observation.terminal,
                "elapsedMicros": observation.elapsed_micros,
                "processLaunched": false,
                "probeProcessLaunched": observation.probe_process_launched,
                "timeout": observation.terminal == "probe-timeout",
                "policyFastPath": !observation.probe_process_launched,
                "cleanupVerified": observation.cleanup_verified,
                "cacheHit": observation.cache_hit,
                "behaviorKey": observation.behavior_key,
            }),
        );
    }
    Ok(())
}
