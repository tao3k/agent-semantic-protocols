//! Minimal Reader behavior observation shared by the Hook compiler and evaluator.

use serde_json::Value;

#[path = "reader_probe_runtime.rs"]
mod runtime;

#[doc(hidden)]
pub fn diagnose_reader_probe(
    command_tokens: Vec<String>,
    subject: String,
) -> Option<ReaderProbeObservation> {
    runtime::observe(&runtime::ReaderProbeRequest {
        command_tokens,
        subject,
    })
}

pub(crate) const READER_PROBE_FIELD: &str = "_aspReaderProbe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReaderProbeAccess {
    Read,
    NotRead,
    Unknown,
}

/// Classify an `open`/`openat` access-mode flag without command-name knowledge.
///
/// Only read-only authority is projected as Reader. `O_RDWR` remains NotRead
/// because Codex has a distinct native Edit route. Linux `O_PATH` is not a
/// content read even though its access-mode bits are zero like `O_RDONLY`.
pub const fn classify_open_access_mode(flags: i32) -> ReaderProbeAccess {
    #[cfg(target_os = "linux")]
    if flags & libc::O_PATH != 0 {
        return ReaderProbeAccess::NotRead;
    }
    match flags & libc::O_ACCMODE {
        libc::O_RDONLY => ReaderProbeAccess::Read,
        libc::O_WRONLY | libc::O_RDWR => ReaderProbeAccess::NotRead,
        _ => ReaderProbeAccess::Unknown,
    }
}

impl ReaderProbeAccess {
    const fn label(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::NotRead => "not-read",
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
}

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn reader_probe_interposer_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/asp-reader-probe.dylib"))
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
                    ReaderProbeAccess::Read => "O_RDONLY",
                    ReaderProbeAccess::NotRead => "not-read-only",
                    ReaderProbeAccess::Unknown => "unknown",
                },
                "backend": observation.backend,
                "syscall": (observation.terminal == "open-entry-observed")
                    .then_some("open/openat"),
                "terminal": observation.terminal,
                "elapsedMicros": observation.elapsed_micros,
                "processLaunched": false,
                "probeProcessLaunched": observation.probe_process_launched,
                "timeout": observation.terminal == "probe-timeout",
                "policyFastPath": !observation.probe_process_launched,
                "cleanupVerified": observation.cleanup_verified,
            }),
        );
    }
    Ok(())
}
