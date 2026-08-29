//! Compiler-facing Reader policy projection and fixture support.

#[cfg(feature = "compiler")]
use serde_json::Value;

#[path = "reader_probe_core.rs"]
mod core;

pub use core::{
    ReaderProbeAccess, ReaderProbeObservation, bind_reader_probe_observation,
    classify_open_access_mode, diagnose_reader_probe, diagnose_reader_probe_with_state_home,
};

#[cfg(target_os = "macos")]
pub use core::reader_probe_interposer_bytes;

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn reader_probe_fixture_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/asp-reader-probe-fixture"))
}

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn materialize_reader_probe_fixture() -> Result<std::path::PathBuf, String> {
    fixture::materialize()
}

#[cfg(feature = "compiler")]
pub(crate) fn observed_reader_subject(tool_input: &Value) -> Option<&str> {
    let observation = tool_input.get(core::READER_PROBE_FIELD)?;
    if observation.get("access")?.as_str()? != "read" {
        return None;
    }
    observation.get("subject")?.as_str()
}

#[cfg(target_os = "macos")]
#[path = "reader_probe_fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "../tests/unit/reader_probe.rs"]
mod tests;
