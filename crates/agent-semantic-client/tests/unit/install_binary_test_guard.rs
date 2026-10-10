// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Serializes production binary and Runtime lifecycle fixtures.
//!
//! Each fixture owns an isolated State Home, while all of them execute the same
//! production artifact and spawn real child processes. Cross-fixture process
//! pressure is not a supported concurrency model; individual scenarios own the
//! explicit concurrency they intend to qualify.

pub(crate) fn acquire() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
