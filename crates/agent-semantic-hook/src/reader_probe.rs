// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compiler-facing Reader policy projection and fixture support.

#[cfg(feature = "compiler")]
use serde_json::Value;

#[path = "reader_probe_core.rs"]
mod core;

pub use core::ReaderProbeAccess;
pub use core::ReaderProbeObservation;
pub use core::bind_reader_probe_observation;
pub use core::diagnose_reader_probe;
pub use core::diagnose_reader_probe_with_state_home;

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
