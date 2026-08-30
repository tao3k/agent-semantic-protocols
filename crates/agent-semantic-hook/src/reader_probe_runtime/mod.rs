//! Owns the bounded Reader probe runtime and its child-process lifecycle.

#[cfg(target_os = "macos")]
mod filesystem;
#[cfg(target_os = "macos")]
mod process;
mod runtime;

pub(super) use runtime::{ReaderProbeRequest, observe};
