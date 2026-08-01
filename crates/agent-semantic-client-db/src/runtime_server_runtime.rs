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
        Self {
            builder: tokio::runtime::Builder::new_current_thread(),
        }
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
