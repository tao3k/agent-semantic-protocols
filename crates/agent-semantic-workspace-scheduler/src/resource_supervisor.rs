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
    cpu: std::sync::Arc<tokio::sync::Semaphore>,
    memory: std::sync::Arc<tokio::sync::Semaphore>,
    effective_cpu: usize,
    background_cpu: usize,
    memory_budget_bytes: usize,
    memory_units: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeServerResourceRequest {
    pub cpu: usize,
    pub memory_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerResourcePermitReceipt {
    pub effective_cpu: usize,
    pub background_cpu: usize,
    pub admitted_cpu: usize,
    pub process_memory_budget_bytes: usize,
    pub admitted_memory_bytes: usize,
    pub queue_wait_micros: u64,
}

pub struct RuntimeServerResourcePermit {
    _cpu: tokio::sync::OwnedSemaphorePermit,
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
        let memory_units = memory_budget_bytes
            .max(1)
            .div_ceil(MEMORY_PERMIT_UNIT_BYTES)
            .min(u32::MAX as usize);
        Self {
            cpu: std::sync::Arc::new(tokio::sync::Semaphore::new(background_cpu)),
            memory: std::sync::Arc::new(tokio::sync::Semaphore::new(memory_units)),
            effective_cpu,
            background_cpu,
            memory_budget_bytes: memory_budget_bytes.max(1),
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
    pub fn active_background_cpu(&self) -> usize {
        self.background_cpu
            .saturating_sub(self.cpu.available_permits())
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
        let memory_permits = u32::try_from(memory_units)
            .map_err(|_| "Runtime Server memory permit count overflows".to_owned())?;
        let started = std::time::Instant::now();
        let cpu = std::sync::Arc::clone(&self.cpu)
            .acquire_many_owned(cpu_permits)
            .await
            .map_err(|_| "Runtime Server CPU resource authority is closed".to_owned())?;
        let memory = std::sync::Arc::clone(&self.memory)
            .acquire_many_owned(memory_permits)
            .await
            .map_err(|_| "Runtime Server memory resource authority is closed".to_owned())?;
        Ok(RuntimeServerResourcePermit {
            _cpu: cpu,
            _memory: memory,
            receipt: RuntimeServerResourcePermitReceipt {
                effective_cpu: self.effective_cpu,
                background_cpu: self.background_cpu,
                admitted_cpu: request.cpu,
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
}
