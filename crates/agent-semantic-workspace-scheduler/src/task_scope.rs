// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

/// Join-owned daemon task boundary for Runtime Server background actors.
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
        let permit = RuntimeServerTaskPermit {
            lifecycle: std::sync::Arc::clone(&self.lifecycle),
            terminal_recorded: false,
        };
        Ok(RuntimeServerOwnedTask {
            name,
            handle: Some(tokio::task::spawn_blocking(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation));
                match outcome {
                    Ok(value) => {
                        permit.complete();
                        value
                    }
                    Err(payload) => {
                        permit.fail();
                        std::panic::resume_unwind(payload)
                    }
                }
            })),
            lifecycle: std::sync::Arc::clone(&self.lifecycle),
            // The blocking closure owns the lifecycle permit because aborting
            // its JoinHandle cannot cancel work that has already started.
            terminal_recorded: true,
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
        if self.terminal_recorded {
            return;
        }
        self.terminal_recorded = true;
        record_terminal(&self.lifecycle, outcome);
    }
}

impl Drop for RuntimeServerTaskPermit {
    fn drop(&mut self) {
        if !self.terminal_recorded {
            self.terminal_recorded = true;
            record_terminal(&self.lifecycle, RuntimeServerTaskTerminalOutcome::Cancelled);
        }
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
        if self.terminal_recorded {
            return;
        }
        self.terminal_recorded = true;
        record_terminal(&self.lifecycle, outcome);
    }
}

enum RuntimeServerTaskTerminalOutcome {
    Completed,
    Cancelled,
    Failed,
}

fn record_terminal(
    lifecycle: &RuntimeServerTaskLifecycle,
    outcome: RuntimeServerTaskTerminalOutcome,
) {
    use std::sync::atomic::Ordering;
    lifecycle.active.fetch_sub(1, Ordering::AcqRel);
    match outcome {
        RuntimeServerTaskTerminalOutcome::Completed => {
            lifecycle.completed.fetch_add(1, Ordering::AcqRel);
        }
        RuntimeServerTaskTerminalOutcome::Cancelled => {
            lifecycle.cancelled.fetch_add(1, Ordering::AcqRel);
        }
        RuntimeServerTaskTerminalOutcome::Failed => {
            lifecycle.failed.fetch_add(1, Ordering::AcqRel);
        }
    }
}

impl<T> Drop for RuntimeServerOwnedTask<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            self.record_terminal(RuntimeServerTaskTerminalOutcome::Cancelled);
        }
    }
}
