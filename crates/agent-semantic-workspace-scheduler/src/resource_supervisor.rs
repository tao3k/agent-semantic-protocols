// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

const MEMORY_BUDGET_FALLBACK_BYTES: usize = 256 * 1024 * 1024;
const MEMORY_PERMIT_UNIT_BYTES: usize = 1024 * 1024;

#[must_use]
pub fn runtime_server_process_memory_budget_bytes() -> usize {
    effective_process_memory_capacity_bytes()
        .filter(|capacity| *capacity > 0)
        .unwrap_or(MEMORY_BUDGET_FALLBACK_BYTES)
}

#[must_use]
fn effective_process_memory_capacity_bytes() -> Option<usize> {
    #[cfg(target_os = "linux")]
    if let Ok(limit) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
        let limit = limit.trim();
        if limit != "max"
            && let Ok(bytes) = limit.parse::<usize>()
            && bytes > 0
        {
            return Some(bytes);
        }
    }

    #[cfg(unix)]
    {
        let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if pages > 0 && page_size > 0 {
            return usize::try_from(pages).ok().and_then(|pages| {
                usize::try_from(page_size)
                    .ok()
                    .and_then(|page_size| pages.checked_mul(page_size))
            });
        }
    }

    None
}

#[derive(Clone)]
pub struct RuntimeServerResourceSupervisor {
    queue: std::sync::Arc<tokio::sync::Semaphore>,
    work: std::sync::Arc<tokio::sync::Semaphore>,
    cpu: std::sync::Arc<tokio::sync::Semaphore>,
    memory: std::sync::Arc<tokio::sync::Semaphore>,
    effective_cpu: usize,
    background_cpu: usize,
    queue_capacity: usize,
    work_byte_budget: usize,
    work_units: usize,
    memory_budget_bytes: usize,
    memory_units: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeServerResourceRequest {
    pub cpu: usize,
    pub work_bytes: usize,
    pub memory_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerResourcePermitReceipt {
    pub effective_cpu: usize,
    pub background_cpu: usize,
    pub queue_capacity: usize,
    pub admitted_cpu: usize,
    pub process_work_byte_budget: usize,
    pub admitted_work_bytes: usize,
    pub process_memory_budget_bytes: usize,
    pub admitted_memory_bytes: usize,
    pub queue_wait_micros: u64,
}

pub struct RuntimeServerResourcePermit {
    cpu: Option<tokio::sync::OwnedSemaphorePermit>,
    _work: tokio::sync::OwnedSemaphorePermit,
    _memory: tokio::sync::OwnedSemaphorePermit,
    receipt: RuntimeServerResourcePermitReceipt,
}

impl RuntimeServerResourceSupervisor {
    pub fn for_current_daemon() -> Self {
        let effective_cpu = tokio::runtime::Handle::current()
            .metrics()
            .num_workers()
            .max(1);
        Self::new(effective_cpu, runtime_server_process_memory_budget_bytes())
    }

    pub fn new(effective_cpu: usize, memory_budget_bytes: usize) -> Self {
        let effective_cpu = effective_cpu.max(1);
        let background_cpu = effective_cpu.saturating_sub(1).max(1);
        let queue_capacity = background_cpu;
        let memory_budget_bytes = memory_budget_bytes.max(1);
        let requested_work_byte_budget = memory_budget_bytes.saturating_mul(background_cpu);
        let work_units = requested_work_byte_budget
            .div_ceil(MEMORY_PERMIT_UNIT_BYTES)
            .min(u32::MAX as usize);
        let work_byte_budget =
            requested_work_byte_budget.min(work_units.saturating_mul(MEMORY_PERMIT_UNIT_BYTES));
        let memory_units = memory_budget_bytes
            .div_ceil(MEMORY_PERMIT_UNIT_BYTES)
            .min(u32::MAX as usize);
        Self {
            queue: std::sync::Arc::new(tokio::sync::Semaphore::new(queue_capacity)),
            work: std::sync::Arc::new(tokio::sync::Semaphore::new(work_units)),
            cpu: std::sync::Arc::new(tokio::sync::Semaphore::new(background_cpu)),
            memory: std::sync::Arc::new(tokio::sync::Semaphore::new(memory_units)),
            effective_cpu,
            background_cpu,
            queue_capacity,
            work_byte_budget,
            work_units,
            memory_budget_bytes,
            memory_units,
        }
    }

    #[must_use]
    pub fn effective_cpu(&self) -> usize {
        self.effective_cpu
    }

    #[must_use]
    pub fn background_cpu(&self) -> usize {
        self.background_cpu
    }

    #[must_use]
    pub fn memory_budget_bytes(&self) -> usize {
        self.memory_budget_bytes
    }

    #[must_use]
    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    #[must_use]
    pub fn work_byte_budget(&self) -> usize {
        self.work_byte_budget
    }

    #[must_use]
    pub fn active_queue_depth(&self) -> usize {
        self.queue_capacity
            .saturating_sub(self.queue.available_permits())
    }

    #[must_use]
    pub fn active_work_bytes(&self) -> usize {
        self.work_units
            .saturating_sub(self.work.available_permits())
            .saturating_mul(MEMORY_PERMIT_UNIT_BYTES)
    }

    #[must_use]
    pub fn active_background_cpu(&self) -> usize {
        self.background_cpu
            .saturating_sub(self.cpu.available_permits())
    }

    #[must_use]
    pub fn active_memory_bytes(&self) -> usize {
        self.memory_units
            .saturating_sub(self.memory.available_permits())
            .saturating_mul(MEMORY_PERMIT_UNIT_BYTES)
    }

    pub async fn acquire(
        &self,
        request: RuntimeServerResourceRequest,
    ) -> Result<RuntimeServerResourcePermit, String> {
        if request.cpu == 0 || request.cpu > self.background_cpu {
            return Err(
                "Runtime Server resource request exceeds background CPU authority".to_owned(),
            );
        }
        if request.work_bytes == 0 || request.work_bytes > self.work_byte_budget {
            return Err(format!(
                "Runtime Server work-byte request exceeds process authority: requested={} budget={}",
                request.work_bytes, self.work_byte_budget
            ));
        }
        let memory_units = request
            .memory_bytes
            .max(1)
            .div_ceil(MEMORY_PERMIT_UNIT_BYTES);
        if request.memory_bytes > self.memory_budget_bytes || memory_units > self.memory_units {
            return Err(
                "Runtime Server resource request exceeds process memory authority".to_owned(),
            );
        }
        let cpu_permits = u32::try_from(request.cpu)
            .map_err(|_| "Runtime Server CPU permit count overflows".to_owned())?;
        let work_units = request.work_bytes.div_ceil(MEMORY_PERMIT_UNIT_BYTES);
        let work_permits = u32::try_from(work_units)
            .map_err(|_| "Runtime Server work-byte permit count overflows".to_owned())?;
        let memory_permits = u32::try_from(memory_units)
            .map_err(|_| "Runtime Server memory permit count overflows".to_owned())?;
        let started = std::time::Instant::now();
        // The queue is intentionally non-waiting: admitted waiters are bounded
        // by executable background lanes, so contention cannot accumulate an
        // unbounded future/task population. The slot remains held until both
        // input work bytes, retained memory, and CPU have been acquired.
        let queue = std::sync::Arc::clone(&self.queue)
            .try_acquire_owned()
            .map_err(|_| {
                format!(
                    "Runtime Server resource queue is saturated: capacity={}",
                    self.queue_capacity
                )
            })?;
        // Byte authorities precede CPU. A request waiting for either work or
        // retained memory must never hoard CPU capacity needed by an admitted
        // stage to finish and release its permits.
        let work = std::sync::Arc::clone(&self.work)
            .acquire_many_owned(work_permits)
            .await
            .map_err(|_| "Runtime Server work-byte resource authority is closed".to_owned())?;
        let memory = std::sync::Arc::clone(&self.memory)
            .acquire_many_owned(memory_permits)
            .await
            .map_err(|_| "Runtime Server memory resource authority is closed".to_owned())?;
        let cpu = std::sync::Arc::clone(&self.cpu)
            .acquire_many_owned(cpu_permits)
            .await
            .map_err(|_| "Runtime Server CPU resource authority is closed".to_owned())?;
        drop(queue);
        Ok(RuntimeServerResourcePermit {
            cpu: Some(cpu),
            _work: work,
            _memory: memory,
            receipt: RuntimeServerResourcePermitReceipt {
                effective_cpu: self.effective_cpu,
                background_cpu: self.background_cpu,
                queue_capacity: self.queue_capacity,
                admitted_cpu: request.cpu,
                process_work_byte_budget: self.work_byte_budget,
                admitted_work_bytes: request.work_bytes,
                process_memory_budget_bytes: self.memory_budget_bytes,
                admitted_memory_bytes: memory_units.saturating_mul(MEMORY_PERMIT_UNIT_BYTES),
                queue_wait_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
            },
        })
    }
}

impl RuntimeServerResourcePermit {
    #[must_use]
    pub fn receipt(&self) -> RuntimeServerResourcePermitReceipt {
        self.receipt
    }

    /// Release execution capacity after CPU work finishes while retaining the
    /// memory reservation for result data that remains live downstream.
    pub fn release_cpu(&mut self) {
        self.cpu.take();
    }
}
