// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owns the bounded Reader probe runtime and its child-process lifecycle.

#[cfg(target_os = "macos")]
mod filesystem;
#[path = "runtime.rs"]
mod implementation;
#[cfg(target_os = "macos")]
mod process;

pub(super) use implementation::ReaderProbeRequest;
pub(super) use implementation::observe;
