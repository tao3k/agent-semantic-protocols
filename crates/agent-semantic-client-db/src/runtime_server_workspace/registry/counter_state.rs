// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::atomic::Ordering;

use super::{RuntimeDataPlaneCounterState, RuntimeDataPlaneCounters};

impl RuntimeDataPlaneCounterState {
    pub(super) fn snapshot(&self) -> RuntimeDataPlaneCounters {
        RuntimeDataPlaneCounters {
            filesystem_reads: self.filesystem_reads.load(Ordering::Relaxed),
            filesystem_writes: self.filesystem_writes.load(Ordering::Relaxed),
            ..RuntimeDataPlaneCounters::default()
        }
    }
}
