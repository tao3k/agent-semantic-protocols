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
}

impl<T> RuntimeServerOwnedTask<T>
where
    T: Send + 'static,
{
    pub fn spawn<F>(name: &'static str, future: F) -> Self
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        Self {
            name,
            handle: Some(tokio::spawn(future)),
        }
    }

    pub fn spawn_blocking<F>(name: &'static str, operation: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        Self {
            name,
            handle: Some(tokio::task::spawn_blocking(operation)),
        }
    }

    pub async fn join(mut self) -> Result<T, String> {
        self.handle
            .take()
            .ok_or_else(|| format!("Runtime Server task `{}` has no join handle", self.name))?
            .await
            .map_err(|error| format!("Runtime Server task `{}` failed: {error}", self.name))
    }

    pub fn abort(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl<T> Drop for RuntimeServerOwnedTask<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
