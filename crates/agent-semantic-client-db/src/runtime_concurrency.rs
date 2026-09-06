// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeConcurrencyPlan {
    worker_count: usize,
    runnable_task_count: usize,
}

impl RuntimeConcurrencyPlan {
    pub(crate) fn current() -> Self {
        tokio::runtime::Handle::try_current()
            .map(|runtime| {
                let metrics = runtime.metrics();
                Self {
                    worker_count: metrics.num_workers().max(1),
                    runnable_task_count: metrics.global_queue_depth(),
                }
            })
            .unwrap_or_else(|_| Self {
                worker_count: std::thread::available_parallelism()
                    .map(usize::from)
                    .unwrap_or(1),
                runnable_task_count: 0,
            })
    }

    pub(crate) fn reader_limit(self) -> usize {
        self.worker_count
    }

    pub(crate) fn ipc_read_lane_capacity(self) -> usize {
        self.worker_count
            .saturating_mul(2)
            .max(self.runnable_task_count)
            .checked_next_power_of_two()
            .unwrap_or(usize::MAX)
            .clamp(1, 256)
    }

    pub(crate) fn writer_batch_limit(self) -> usize {
        let pressure_units = self.runnable_task_count.div_ceil(self.worker_count);
        self.worker_count
            .saturating_add(pressure_units)
            .checked_next_power_of_two()
            .unwrap_or(usize::MAX)
    }

    pub(crate) fn writer_queue_capacity(self) -> usize {
        let batch_limit = self.writer_batch_limit();
        batch_limit
            .saturating_mul(self.worker_count)
            .max(batch_limit)
    }
}
