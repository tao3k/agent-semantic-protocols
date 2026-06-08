//! Shared CLI version rendering.

pub(in crate::command) fn protocol_version_line() -> String {
    format!("asp {}", env!("CARGO_PKG_VERSION"))
}
