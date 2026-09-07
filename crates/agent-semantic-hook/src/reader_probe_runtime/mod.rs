// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owns the bounded Reader probe runtime and its child-process lifecycle.

#[cfg(target_os = "macos")]
mod filesystem;
#[cfg(target_os = "macos")]
mod process;
mod runtime;

pub(super) use runtime::ReaderProbeRequest;
pub(super) use runtime::observe;
