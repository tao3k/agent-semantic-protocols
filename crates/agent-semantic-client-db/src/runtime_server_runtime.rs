use std::future::Future;
use std::sync::LazyLock;

pub struct RuntimeServerRuntime {
    runtime: tokio::runtime::Runtime,
}

static CLIENT_RUNTIME: LazyLock<Result<RuntimeServerRuntime, String>> = LazyLock::new(|| {
    RuntimeServerRuntimeBuilder::new_client()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create Runtime Server client Tokio runtime: {error}"))
});

pub struct RuntimeServerClientExecutor;

impl RuntimeServerClientExecutor {
    pub fn get() -> Result<&'static RuntimeServerRuntime, String> {
        match &*CLIENT_RUNTIME {
            Ok(runtime) => Ok(runtime),
            Err(error) => Err(error.clone()),
        }
    }
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

pub const RUNTIME_SERVER_CLIENT_WORKER_COUNT: usize = 2;
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
    pub fn new_daemon() -> Self {
        let worker_count = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(2)
            .max(2);
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(worker_count);
        Self { builder }
    }

    pub fn new_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(RUNTIME_SERVER_CLIENT_WORKER_COUNT)
            .thread_name("asp-runtime-client");
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
