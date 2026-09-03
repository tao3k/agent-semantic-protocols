use std::future::Future;

pub struct RuntimeServerRuntime {
    runtime: tokio::runtime::Runtime,
}

impl RuntimeServerRuntime {
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        self.runtime.block_on(future)
    }
}

pub struct RuntimeServerRuntimeBuilder {
    builder: tokio::runtime::Builder,
}

/// Effective CPU capacity exposed to this process. The OS value accounts for
/// processor-set and cgroup limits, so interactive clients and the daemon Tokio runtimes
/// share one host-authoritative capacity policy rather than fixed products
/// defaults.
pub fn adaptive_tokio_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

const RUNTIME_SERVER_MEMORY_BUDGET_FALLBACK_BYTES: usize = 256 * 1024 * 1024;

/// Process-memory authority used by resident Runtime services.
///
/// The ceiling is derived once from the effective cgroup/host capacity. Actual
/// background consumption is controlled by daemon-wide typed permits; Search
/// never probes the host or invents a private fraction or fixed cap.
#[must_use]
pub fn runtime_server_process_memory_budget_bytes() -> usize {
    effective_process_memory_capacity_bytes()
        .filter(|capacity| *capacity > 0)
        .unwrap_or(RUNTIME_SERVER_MEMORY_BUDGET_FALLBACK_BYTES)
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

const RUNTIME_SERVER_MEMORY_PERMIT_UNIT_BYTES: usize = 1024 * 1024;

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
            .div_ceil(RUNTIME_SERVER_MEMORY_PERMIT_UNIT_BYTES)
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
            .div_ceil(RUNTIME_SERVER_MEMORY_PERMIT_UNIT_BYTES);
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
                admitted_memory_bytes: memory_units
                    .saturating_mul(RUNTIME_SERVER_MEMORY_PERMIT_UNIT_BYTES),
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

pub const RUNTIME_SERVER_CONNECTION_IO_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(25);

pub async fn within_connection_io_budget<T>(
    surface: &'static str,
    operation: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(RUNTIME_SERVER_CONNECTION_IO_BUDGET, operation)
        .await
        .map_err(|_| {
            format!(
                "Runtime Server {surface} exceeded the {}ms connection I/O budget",
                RUNTIME_SERVER_CONNECTION_IO_BUDGET.as_millis()
            )
        })?
}

/// Adaptive upper bound for accepted-but-not-reaped resident connections.
///
/// The bound follows the daemon scheduler instead of fixing one machine-wide
/// value. A lease remains owned by the `JoinSet` result until it is reaped, so
/// completed tasks cannot accumulate behind a busy accept branch.
pub fn runtime_server_connection_limit(worker_count: usize) -> usize {
    worker_count.saturating_mul(16).clamp(32, 512)
}

#[derive(Clone)]
pub struct RuntimeServerConnectionSupervisor {
    surface: &'static str,
    limit: usize,
    active: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    high_watermark: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    rejected: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerConnectionSnapshot {
    pub active: usize,
    pub limit: usize,
    pub high_watermark: usize,
    pub rejected: u64,
}

pub struct RuntimeServerConnectionLease {
    supervisor: RuntimeServerConnectionSupervisor,
}

impl RuntimeServerConnectionSupervisor {
    pub fn for_current_runtime(surface: &'static str) -> Self {
        let workers = tokio::runtime::Handle::current().metrics().num_workers();
        Self::new(surface, runtime_server_connection_limit(workers))
    }

    pub fn new(surface: &'static str, limit: usize) -> Self {
        assert!(
            limit > 0,
            "Runtime Server connection limit must be non-zero"
        );
        Self {
            surface,
            limit,
            active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            high_watermark: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            rejected: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Acquire) < self.limit
    }

    pub fn try_admit(&self) -> Option<RuntimeServerConnectionLease> {
        use std::sync::atomic::Ordering;
        let admitted = self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.limit).then_some(active + 1)
            });
        let active = match admitted {
            Ok(previous) => previous + 1,
            Err(_) => {
                self.rejected.fetch_add(1, Ordering::AcqRel);
                self.record_pressure("connection-rejected");
                return None;
            }
        };
        let previous_high = self.high_watermark.fetch_max(active, Ordering::AcqRel);
        if active > previous_high {
            self.record_pressure("connection-high-watermark");
        }
        Some(RuntimeServerConnectionLease {
            supervisor: self.clone(),
        })
    }

    pub fn snapshot(&self) -> RuntimeServerConnectionSnapshot {
        use std::sync::atomic::Ordering;
        RuntimeServerConnectionSnapshot {
            active: self.active.load(Ordering::Acquire),
            limit: self.limit,
            high_watermark: self.high_watermark.load(Ordering::Acquire),
            rejected: self.rejected.load(Ordering::Acquire),
        }
    }

    fn record_pressure(&self, stage: &'static str) {
        let snapshot = self.snapshot();
        let mut observation =
            crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                self.surface,
                stage,
                0,
                1,
                if snapshot.rejected == 0 {
                    "within-budget"
                } else {
                    "budget-exceeded"
                },
            );
        observation.runtime_active_connections = Some(snapshot.active as u64);
        observation.runtime_connection_limit = Some(snapshot.limit as u64);
        observation.runtime_connection_high_watermark = Some(snapshot.high_watermark as u64);
        observation.runtime_rejected_connections = Some(snapshot.rejected);
        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
    }
}

impl Drop for RuntimeServerConnectionLease {
    fn drop(&mut self) {
        self.supervisor
            .active
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

impl RuntimeServerRuntimeBuilder {
    pub fn new_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(adaptive_tokio_worker_count())
            .thread_name("asp-client");
        Self { builder }
    }

    /// Minimal Tokio scheduler for one Host Hook IPC request.
    ///
    /// The resident Runtime Server owns discovery, generation builds, and
    /// publication. A Hook process only parses one framed event and submits a
    /// typed request, so constructing the interactive multi-thread client pool
    /// here would put client startup on every PostTool critical path.
    pub fn new_hook_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.thread_name("asp-hook-client");
        Self { builder }
    }

    /// Minimal server-first CLI runtime for an explicitly authorized
    /// `ASP_NO_AGENT=1` recovery invocation.
    ///
    /// This lane retains only the I/O driver needed to reach the resident ASP
    /// Server. It creates no worker pool; the binary also omits signal-driver
    /// registration so Host sandboxes still have one bounded IPC route.
    pub fn new_no_agent_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.thread_name("asp-no-agent-client");
        Self { builder }
    }

    pub fn new_daemon() -> Self {
        // Tokio's runtime must reflect the host's effective CPU allocation
        // (including cgroup / processor-set limits), not a repository-fixed
        // worker ceiling. `available_parallelism` is that portable OS view.
        let worker_count = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(2);
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        // The Runtime Server is the sole heavy-I/O authority.  Its blocking
        // lane must therefore be bounded by the same scheduler budget as its
        // async workers; Tokio's default (512) can otherwise turn a burst of
        // workspace writes/provider process work into machine-wide I/O
        // contention while the endpoint still appears healthy.
        builder
            .worker_threads(worker_count)
            .max_blocking_threads(worker_count)
            .thread_name("asp-runtime-server");
        Self { builder }
    }

    pub fn enable_all(&mut self) -> &mut Self {
        self.builder.enable_all();
        self
    }

    pub fn build(&mut self) -> Result<RuntimeServerRuntime, std::io::Error> {
        self.builder
            .build()
            .map(|runtime| RuntimeServerRuntime { runtime })
    }
}

/// Join-owned daemon task boundary for Runtime Server background actors.
///
/// Construction, cancellation, and joining stay visible through one typed
/// owner instead of leaking detached `tokio::spawn` handles across services.
pub struct RuntimeServerOwnedTask<T> {
    name: &'static str,
    handle: Option<tokio::task::JoinHandle<T>>,
    lifecycle: std::sync::Arc<RuntimeServerTaskLifecycle>,
    terminal_recorded: bool,
}

const TASK_SCOPE_ACCEPTING: u8 = 0;
const TASK_SCOPE_DRAINING: u8 = 1;
const TASK_SCOPE_TERMINATED: u8 = 2;

struct RuntimeServerTaskLifecycle {
    scope: &'static str,
    state: std::sync::atomic::AtomicU8,
    started: std::sync::atomic::AtomicU64,
    completed: std::sync::atomic::AtomicU64,
    cancelled: std::sync::atomic::AtomicU64,
    failed: std::sync::atomic::AtomicU64,
    active: std::sync::atomic::AtomicU64,
}

/// One ownership boundary for a set of Tokio tasks.
///
/// Admission closes exactly once.  Every task returned by this scope records a
/// terminal outcome on join, explicit abort, or drop, so shutdown can prove
/// that no task was detached from its resident component.
#[derive(Clone)]
pub struct RuntimeServerTaskScope {
    lifecycle: std::sync::Arc<RuntimeServerTaskLifecycle>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerTaskLifecycleReceipt {
    schema_id: String,
    schema_version: String,
    pub scope: String,
    pub state: String,
    pub started: u64,
    pub completed: u64,
    pub cancelled: u64,
    pub failed: u64,
    pub active: u64,
    pub leaked: u64,
    pub drain_micros: u64,
}

pub struct RuntimeServerTaskPermit {
    lifecycle: std::sync::Arc<RuntimeServerTaskLifecycle>,
    terminal_recorded: bool,
}

impl RuntimeServerTaskScope {
    pub fn new(scope: &'static str) -> Self {
        Self {
            lifecycle: std::sync::Arc::new(RuntimeServerTaskLifecycle {
                scope,
                state: std::sync::atomic::AtomicU8::new(TASK_SCOPE_ACCEPTING),
                started: std::sync::atomic::AtomicU64::new(0),
                completed: std::sync::atomic::AtomicU64::new(0),
                cancelled: std::sync::atomic::AtomicU64::new(0),
                failed: std::sync::atomic::AtomicU64::new(0),
                active: std::sync::atomic::AtomicU64::new(0),
            }),
        }
    }

    pub fn spawn<T, F>(
        &self,
        name: &'static str,
        future: F,
    ) -> Result<RuntimeServerOwnedTask<T>, String>
    where
        T: Send + 'static,
        F: std::future::Future<Output = T> + Send + 'static,
    {
        self.admit(name)?;
        Ok(RuntimeServerOwnedTask {
            name,
            handle: Some(tokio::spawn(future)),
            lifecycle: std::sync::Arc::clone(&self.lifecycle),
            terminal_recorded: false,
        })
    }

    pub fn permit(&self, name: &'static str) -> Result<RuntimeServerTaskPermit, String> {
        self.admit(name)?;
        Ok(RuntimeServerTaskPermit {
            lifecycle: std::sync::Arc::clone(&self.lifecycle),
            terminal_recorded: false,
        })
    }

    pub fn spawn_blocking<T, F>(
        &self,
        name: &'static str,
        operation: F,
    ) -> Result<RuntimeServerOwnedTask<T>, String>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.admit(name)?;
        Ok(RuntimeServerOwnedTask {
            name,
            handle: Some(tokio::task::spawn_blocking(operation)),
            lifecycle: std::sync::Arc::clone(&self.lifecycle),
            terminal_recorded: false,
        })
    }

    fn admit(&self, name: &'static str) -> Result<(), String> {
        use std::sync::atomic::Ordering;
        if self.lifecycle.state.load(Ordering::Acquire) != TASK_SCOPE_ACCEPTING {
            return Err(format!(
                "Runtime Server task scope `{}` is draining; task `{name}` was not admitted",
                self.lifecycle.scope
            ));
        }
        self.lifecycle.started.fetch_add(1, Ordering::AcqRel);
        self.lifecycle.active.fetch_add(1, Ordering::AcqRel);
        if self.lifecycle.state.load(Ordering::Acquire) != TASK_SCOPE_ACCEPTING {
            self.lifecycle.started.fetch_sub(1, Ordering::AcqRel);
            self.lifecycle.active.fetch_sub(1, Ordering::AcqRel);
            return Err(format!(
                "Runtime Server task scope `{}` began draining while task `{name}` was admitted",
                self.lifecycle.scope
            ));
        }
        Ok(())
    }

    pub fn begin_drain(&self) {
        use std::sync::atomic::Ordering;
        let _ = self.lifecycle.state.compare_exchange(
            TASK_SCOPE_ACCEPTING,
            TASK_SCOPE_DRAINING,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub fn finish(&self, drain_micros: u64) -> Result<RuntimeServerTaskLifecycleReceipt, String> {
        use std::sync::atomic::Ordering;
        self.begin_drain();
        let active = self.lifecycle.active.load(Ordering::Acquire);
        if active == 0 {
            self.lifecycle
                .state
                .store(TASK_SCOPE_TERMINATED, Ordering::Release);
        }
        let receipt = self.receipt(drain_micros);
        if receipt.active == 0
            && receipt.leaked == 0
            && receipt.started == receipt.completed + receipt.cancelled + receipt.failed
        {
            Ok(receipt)
        } else {
            Err(format!(
                "Runtime Server task scope `{}` did not drain: started={} completed={} cancelled={} failed={} active={} leaked={}",
                receipt.scope,
                receipt.started,
                receipt.completed,
                receipt.cancelled,
                receipt.failed,
                receipt.active,
                receipt.leaked
            ))
        }
    }

    pub fn receipt(&self, drain_micros: u64) -> RuntimeServerTaskLifecycleReceipt {
        use std::sync::atomic::Ordering;
        let state = match self.lifecycle.state.load(Ordering::Acquire) {
            TASK_SCOPE_ACCEPTING => "accepting",
            TASK_SCOPE_DRAINING => "draining",
            _ => "terminated",
        };
        let active = self.lifecycle.active.load(Ordering::Acquire);
        RuntimeServerTaskLifecycleReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-task-lifecycle-receipt".to_owned(),
            schema_version: "1".to_owned(),
            scope: self.lifecycle.scope.to_owned(),
            state: state.to_owned(),
            started: self.lifecycle.started.load(Ordering::Acquire),
            completed: self.lifecycle.completed.load(Ordering::Acquire),
            cancelled: self.lifecycle.cancelled.load(Ordering::Acquire),
            failed: self.lifecycle.failed.load(Ordering::Acquire),
            active,
            leaked: active,
            drain_micros,
        }
    }
}

impl RuntimeServerTaskPermit {
    pub fn complete(mut self) {
        self.record_terminal(RuntimeServerTaskTerminalOutcome::Completed);
    }

    pub fn fail(mut self) {
        self.record_terminal(RuntimeServerTaskTerminalOutcome::Failed);
    }

    fn record_terminal(&mut self, outcome: RuntimeServerTaskTerminalOutcome) {
        use std::sync::atomic::Ordering;
        if self.terminal_recorded {
            return;
        }
        self.terminal_recorded = true;
        self.lifecycle.active.fetch_sub(1, Ordering::AcqRel);
        match outcome {
            RuntimeServerTaskTerminalOutcome::Completed => {
                self.lifecycle.completed.fetch_add(1, Ordering::AcqRel);
            }
            RuntimeServerTaskTerminalOutcome::Cancelled => {
                self.lifecycle.cancelled.fetch_add(1, Ordering::AcqRel);
            }
            RuntimeServerTaskTerminalOutcome::Failed => {
                self.lifecycle.failed.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}

impl Drop for RuntimeServerTaskPermit {
    fn drop(&mut self) {
        self.record_terminal(RuntimeServerTaskTerminalOutcome::Cancelled);
    }
}

impl<T> RuntimeServerOwnedTask<T>
where
    T: Send + 'static,
{
    pub fn spawn<F>(name: &'static str, future: F) -> Self
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        RuntimeServerTaskScope::new(name)
            .spawn(name, future)
            .expect("a new standalone Runtime Server task scope must admit its first task")
    }

    pub fn spawn_blocking<F>(name: &'static str, operation: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        RuntimeServerTaskScope::new(name)
            .spawn_blocking(name, operation)
            .expect("a new standalone Runtime Server task scope must admit its first task")
    }

    pub async fn join(mut self) -> Result<T, String> {
        let outcome = self
            .handle
            .take()
            .ok_or_else(|| format!("Runtime Server task `{}` has no join handle", self.name))?
            .await;
        self.record_join_outcome(&outcome);
        outcome.map_err(|error| format!("Runtime Server task `{}` failed: {error}", self.name))
    }

    pub fn abort(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            self.record_terminal(RuntimeServerTaskTerminalOutcome::Cancelled);
        }
    }

    fn record_join_outcome(&mut self, outcome: &Result<T, tokio::task::JoinError>) {
        match outcome {
            Ok(_) => self.record_terminal(RuntimeServerTaskTerminalOutcome::Completed),
            Err(error) if error.is_cancelled() => {
                self.record_terminal(RuntimeServerTaskTerminalOutcome::Cancelled)
            }
            Err(_) => self.record_terminal(RuntimeServerTaskTerminalOutcome::Failed),
        }
    }
}

impl<T> RuntimeServerOwnedTask<T> {
    fn record_terminal(&mut self, outcome: RuntimeServerTaskTerminalOutcome) {
        use std::sync::atomic::Ordering;
        if self.terminal_recorded {
            return;
        }
        self.terminal_recorded = true;
        self.lifecycle.active.fetch_sub(1, Ordering::AcqRel);
        match outcome {
            RuntimeServerTaskTerminalOutcome::Completed => {
                self.lifecycle.completed.fetch_add(1, Ordering::AcqRel);
            }
            RuntimeServerTaskTerminalOutcome::Cancelled => {
                self.lifecycle.cancelled.fetch_add(1, Ordering::AcqRel);
            }
            RuntimeServerTaskTerminalOutcome::Failed => {
                self.lifecycle.failed.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}

enum RuntimeServerTaskTerminalOutcome {
    Completed,
    Cancelled,
    Failed,
}

impl<T> Drop for RuntimeServerOwnedTask<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            self.record_terminal(RuntimeServerTaskTerminalOutcome::Cancelled);
        }
    }
}
/// Borrows the caller's Tokio runtime for the short-lived binary-install
/// control path. It deliberately owns no runtime, threads, or global state.
/// Runtime Server I/O remains server-owned; this only prevents a nested client
/// runtime while legacy synchronous callers are migrated to async APIs.
pub struct RuntimeServerClientExecutor(tokio::runtime::Handle);

impl RuntimeServerClientExecutor {
    pub fn get() -> Result<Self, String> {
        tokio::runtime::Handle::try_current()
            .map(Self)
            .map_err(|error| format!("ASP caller Tokio runtime is unavailable: {error}"))
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        let handle = self.0.clone();
        tokio::task::block_in_place(move || handle.block_on(future))
    }
}
